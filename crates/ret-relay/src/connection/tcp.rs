/*
 * Copyright (C) 2017 Genymobile
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

use rand::random;
use std::cmp;
use std::io;
use std::net::SocketAddr;
use std::num::Wrapping;
use tokio::net::TcpStream;
use tokio::sync::mpsc::Receiver;
use tokio::task::coop;
use tracing::Level;

use super::id::ConnectionId;
use super::outbox::Outbox;
use crate::binary;
use crate::buffer::StreamBuffer;
use crate::datagram::{TryRead, TryWrite};
use crate::packet::ipv4_packet::{Ipv4Packet, MAX_PACKET_LENGTH};
use crate::packet::packetizer::Packetizer;
use crate::packet::transport::tcp::{self as tcp_header, TcpHeader, TcpHeaderMut};
use crate::packet::transport::{TransportHeader, TransportHeaderMut};
use crate::server::ClientSender;

const TAG: &str = "TcpConnection";

// same value as RetService.MTU in the client
const MTU: u16 = 0x4000;
// 20 bytes for IP headers, 20 bytes for TCP headers
const MAX_PAYLOAD_LENGTH: u16 = MTU - 20 - 20;

pub struct TcpConnection {
    id: ConnectionId,
    // packets for the client, sent in order
    outbox: Outbox,
    client_to_network: StreamBuffer,
    network_to_client: Packetizer,
    closed: bool,
    tcb: Tcb,
}

enum Event {
    Packet(Option<Vec<u8>>),
    Readable(io::Result<()>),
    Writable(io::Result<()>),
    // false if the client is closed
    Flushed(bool),
}

// Transport Control Block
struct Tcb {
    state: TcpState,
    syn_sequence_number: u32,
    sequence_number: Wrapping<u32>,
    acknowledgement_number: Wrapping<u32>,
    their_acknowledgement_number: u32,
    fin_sequence_number: Option<u32>,
    fin_received: bool,
    client_window: u16,
}

// See RFC793: <https://tools.ietf.org/html/rfc793#page-23>
#[derive(Debug, PartialEq, Eq)]
enum TcpState {
    Init,
    SynSent,
    SynReceived,
    Established,
    CloseWait,
    LastAck,
    Closing,
    FinWait1,
    FinWait2,
}

impl TcpState {
    fn is_connected(&self) -> bool {
        self != &TcpState::Init && self != &TcpState::SynSent && self != &TcpState::SynReceived
    }

    fn is_closed(&self) -> bool {
        self == &TcpState::FinWait1
            || self == &TcpState::FinWait2
            || self == &TcpState::Closing
            || self == &TcpState::LastAck
    }
}

impl Tcb {
    fn new() -> Self {
        Self {
            state: TcpState::Init,
            syn_sequence_number: 0,
            sequence_number: Wrapping(0),
            acknowledgement_number: Wrapping(0),
            their_acknowledgement_number: 0,
            fin_sequence_number: None,
            fin_received: false,
            client_window: 0,
        }
    }

    fn remaining_client_window(&self) -> u16 {
        let wrapped_remaining = Wrapping(self.their_acknowledgement_number)
            + Wrapping(u32::from(self.client_window))
            - self.sequence_number;
        let remaining = wrapped_remaining.0;
        if remaining <= u32::from(self.client_window) {
            remaining as u16
        } else {
            0
        }
    }

    fn numbers(&self) -> String {
        format!(
            "(seq={}, ack={})",
            self.sequence_number, self.acknowledgement_number
        )
    }
}

impl TcpConnection {
    /// Relay a TCP connection initiated by the client, until it is closed
    ///
    /// `inbound` receives the next packets sent by the client for this connection.
    pub async fn run(
        id: ConnectionId,
        client: ClientSender,
        mut first_packet: Vec<u8>,
        mut inbound: Receiver<Vec<u8>>,
    ) {
        cx_info!(target: TAG, id, "Open");
        let mut connection = Self::new(id, client, &mut first_packet);
        connection.handle_raw_packet(&mut first_packet);
        if !connection.closed
            && let Some(stream) = connection.connect(&mut inbound).await
        {
            connection.process(&stream, &mut inbound).await;
        }
        // deliver the last packets (RST, ACK of a FIN...) before terminating
        while !connection.outbox.is_empty() && connection.outbox.flush().await {}
        // the network socket is closed when dropped
        cx_info!(target: TAG, connection.id, "Close");
    }

    fn new(id: ConnectionId, client: ClientSender, first_packet: &mut [u8]) -> Self {
        let ipv4_packet = Ipv4Packet::parse(first_packet);
        let (ipv4_header, transport_header) = ipv4_packet.headers();
        let tcp_header = Self::tcp_header_of_transport(transport_header.expect("No transport"));

        // shrink the TCP options to pass a minimal refrence header to the packetizer
        let mut shrinked_tcp_header_raw = [0u8; 20];
        shrinked_tcp_header_raw.copy_from_slice(&tcp_header.raw()[..20]);
        let mut shrinked_tcp_header_data = tcp_header.data().clone();
        {
            let mut shrinked_tcp_header =
                shrinked_tcp_header_data.bind_mut(&mut shrinked_tcp_header_raw);
            shrinked_tcp_header.shrink_options();
            assert_eq!(20, shrinked_tcp_header.header_length());
        }

        let shrinked_transport_header = shrinked_tcp_header_data
            .bind(&shrinked_tcp_header_raw)
            .into();

        Self {
            id,
            outbox: Outbox::new(client),
            client_to_network: StreamBuffer::new(4 * MAX_PACKET_LENGTH),
            network_to_client: Packetizer::new(&ipv4_header, &shrinked_transport_header),
            closed: false,
            tcb: Tcb::new(),
        }
    }

    /// Connect to the destination, while handling the packets the client sends meanwhile
    async fn connect(&mut self, inbound: &mut Receiver<Vec<u8>>) -> Option<TcpStream> {
        let connect = TcpStream::connect(SocketAddr::from(self.id.rewritten_destination()));
        tokio::pin!(connect);
        loop {
            tokio::select! {
                result = &mut connect => {
                    return match result {
                        Ok(stream) => {
                            self.process_connect();
                            Some(stream)
                        }
                        Err(err) => {
                            cx_error!(target: TAG, self.id, "Cannot connect: {}", err);
                            None
                        }
                    };
                }
                packet = inbound.recv() => match packet {
                    Some(mut raw) => {
                        self.handle_raw_packet(&mut raw);
                        if self.closed {
                            return None;
                        }
                    }
                    // the client is closed
                    None => return None,
                },
            }
        }
    }

    async fn process(&mut self, stream: &TcpStream, inbound: &mut Receiver<Vec<u8>>) {
        while !self.closed {
            let may_read = self.may_read();
            let may_write = self.may_write();
            let has_backlog = !self.outbox.is_empty();
            let event = tokio::select! {
                packet = inbound.recv() => Event::Packet(packet),
                result = stream.readable(), if may_read => Event::Readable(result),
                result = stream.writable(), if may_write => Event::Writable(result),
                open = self.outbox.flush(), if has_backlog => Event::Flushed(open),
            };
            match event {
                Event::Packet(Some(mut raw)) => self.handle_raw_packet(&mut raw),
                // the client is closed
                Event::Packet(None) | Event::Flushed(false) => return,
                Event::Readable(Ok(())) => {
                    self.process_receive(stream);
                    // readiness does not consume the task budget, let the other tasks run
                    coop::consume_budget().await;
                }
                Event::Writable(Ok(())) => self.process_send(stream),
                Event::Readable(Err(err)) | Event::Writable(Err(err)) => {
                    cx_error!(target: TAG, self.id, "Socket error: {}", err);
                    self.send_empty_packet_to_client(tcp_header::FLAG_RST);
                    self.close();
                }
                Event::Flushed(true) => (),
            }
        }
    }

    fn close(&mut self) {
        self.closed = true;
    }

    fn process_send(&mut self, stream: &TcpStream) {
        let mut written = 0;
        while !self.client_to_network.is_empty() {
            match self.client_to_network.write_to(&mut TryWrite(stream)) {
                Ok(0) => {
                    cx_error!(target: TAG, self.id, "Cannot write: the socket accepts no data");
                    self.close();
                    return;
                }
                Ok(w) => written += w,
                Err(err) if err.kind() == io::ErrorKind::WouldBlock => break,
                Err(err) => {
                    cx_error!(
                        target: TAG,
                        self.id,
                        "Cannot write: [{:?}] {}",
                        err.kind(),
                        err
                    );
                    self.send_empty_packet_to_client(tcp_header::FLAG_RST);
                    self.close();
                    return;
                }
            }
        }
        if written == 0 {
            return;
        }

        self.tcb.acknowledgement_number += Wrapping(written as u32);
        if self.tcb.fin_received && self.client_to_network.is_empty() {
            cx_debug!(
                target: TAG,
                self.id,
                "No more pending data, process the pending FIN"
            );
            self.do_handle_fin();
        } else {
            cx_debug!(
                target: TAG,
                self.id,
                "Sending ACK {} to client",
                self.tcb.numbers()
            );
            self.send_empty_packet_to_client(tcp_header::FLAG_ACK);
        }
    }

    fn process_receive(&mut self, stream: &TcpStream) {
        assert!(self.outbox.is_empty(), "Packets for the client are waiting");
        let remaining_client_window = self.tcb.remaining_client_window();
        assert!(
            remaining_client_window > 0,
            "process_received() must not be called when window == 0"
        );
        let max_payload_length =
            Some(cmp::min(remaining_client_window, MAX_PAYLOAD_LENGTH) as usize);
        Self::update_headers(
            self.network_to_client.transport_header_mut(),
            &self.tcb,
            tcp_header::FLAG_ACK | tcp_header::FLAG_PSH,
        );
        let packet = match self
            .network_to_client
            .packetize_read(&mut TryRead(stream), max_payload_length)
        {
            Ok(Some(ipv4_packet)) => (
                ipv4_packet.raw().to_vec(),
                ipv4_packet.payload().unwrap().len() as u16,
            ),
            Ok(None) => {
                self.eof();
                return;
            }
            Err(err) if err.kind() == io::ErrorKind::WouldBlock => return,
            Err(err) => {
                cx_error!(
                    target: TAG,
                    self.id,
                    "Cannot read: [{:?}] {}",
                    err.kind(),
                    err
                );
                self.send_empty_packet_to_client(tcp_header::FLAG_RST);
                self.close();
                return;
            }
        };

        let (raw, payload_length) = packet;
        // queued packets are never dropped: they count as sent
        self.tcb.sequence_number += Wrapping(u32::from(payload_length));
        cx_debug!(
            target: TAG,
            self.id,
            "Packet ({} bytes) sent to client {}",
            payload_length,
            self.tcb.numbers()
        );
        // if it does not fit in the client queue, stop reading from the network until it does
        if !self.outbox.push(raw) {
            self.close();
        }
    }

    fn process_connect(&mut self) {
        assert_eq!(self.tcb.state, TcpState::SynSent);
        self.tcb.state = TcpState::SynReceived;
        cx_debug!(target: TAG, self.id, "State = {:?}", self.tcb.state);
        self.send_empty_packet_to_client(tcp_header::FLAG_SYN | tcp_header::FLAG_ACK);
        self.tcb.sequence_number += Wrapping(1); // SYN counts for 1 byte
    }

    fn send_empty_packet_to_client(&mut self, flags: u16) {
        let raw = Self::create_empty_response_packet(
            &self.id,
            &mut self.network_to_client,
            &self.tcb,
            flags,
        )
        .raw()
        .to_vec();
        if !self.outbox.push(raw) {
            self.close();
        }
    }

    fn eof(&mut self) {
        self.send_empty_packet_to_client(tcp_header::FLAG_FIN | tcp_header::FLAG_ACK);
        self.tcb.fin_sequence_number = Some(self.tcb.sequence_number.0);
        self.tcb.sequence_number += Wrapping(1); // FIN counts for 1 byte
        self.tcb.state = if self.tcb.state == TcpState::CloseWait {
            TcpState::LastAck
        } else {
            TcpState::FinWait1
        };
        cx_debug!(target: TAG, self.id, "State = {:?}", self.tcb.state);
    }

    #[inline]
    fn tcp_header_of_transport(transport_header: TransportHeader) -> TcpHeader {
        if let TransportHeader::Tcp(tcp_header) = transport_header {
            tcp_header
        } else {
            panic!("Not a TCP header");
        }
    }

    #[inline]
    fn tcp_header_of_transport_mut(transport_header: TransportHeaderMut) -> TcpHeaderMut {
        if let TransportHeaderMut::Tcp(tcp_header) = transport_header {
            tcp_header
        } else {
            panic!("Not a TCP header");
        }
    }

    #[inline]
    fn tcp_header_of_packet<'a>(ipv4_packet: &'a Ipv4Packet) -> TcpHeader<'a> {
        if let Some(TransportHeader::Tcp(tcp_header)) = ipv4_packet.transport_header() {
            tcp_header
        } else {
            panic!("Not a TCP packet");
        }
    }

    fn update_headers(transport_header: TransportHeaderMut, tcb: &Tcb, flags: u16) {
        let mut tcp_header = Self::tcp_header_of_transport_mut(transport_header);
        tcp_header.set_sequence_number(tcb.sequence_number.0);
        tcp_header.set_acknowledgement_number(tcb.acknowledgement_number.0);
        tcp_header.set_flags(flags);
    }

    fn handle_raw_packet(&mut self, raw: &mut [u8]) {
        let ipv4_packet = Ipv4Packet::parse(raw);
        self.handle_packet(&ipv4_packet);
    }

    fn handle_packet(&mut self, ipv4_packet: &Ipv4Packet) {
        let tcp_header = Self::tcp_header_of_packet(ipv4_packet);
        if self.tcb.state == TcpState::Init {
            self.handle_first_packet(ipv4_packet);
            return;
        }

        if tcp_header.is_syn() {
            self.handle_duplicate_syn(ipv4_packet);
            return;
        }

        let expected_packet =
            (self.tcb.acknowledgement_number + Wrapping(self.client_to_network.size() as u32)).0;
        if tcp_header.sequence_number() != expected_packet {
            // ignore packet already received or out-of-order, retransmission is already
            // managed by both sides
            cx_warn!(
                target: TAG,
                self.id,
                "Ignoring packet {} (acking {}); expecting {}; flags={}",
                tcp_header.sequence_number(),
                tcp_header.acknowledgement_number(),
                expected_packet,
                tcp_header.flags()
            );
            if !tcp_header.is_rst() && self.tcb.state.is_connected() {
                // RFC 793: an unacceptable segment must be answered with an ACK. Keepalive
                // probes (sequence number one below the expected one) rely on it: without a
                // reply, the client aborts the connection after a few probes.
                self.send_empty_packet_to_client(tcp_header::FLAG_ACK);
            }
            return;
        }

        self.tcb.client_window = tcp_header.window();
        self.tcb.their_acknowledgement_number = tcp_header.acknowledgement_number();

        cx_debug!(
            target: TAG,
            self.id,
            "Receiving expected packet {} (flags={})",
            tcp_header.sequence_number(),
            tcp_header.flags()
        );

        if tcp_header.is_rst() {
            self.close();
            return;
        }

        if tcp_header.is_ack() {
            cx_debug!(
                target: TAG,
                self.id,
                "Client acked {}",
                tcp_header.acknowledgement_number()
            );

            self.handle_ack(ipv4_packet);
        }

        if tcp_header.is_fin() {
            self.handle_fin();
        }

        if let Some(fin_sequence_number) = self.tcb.fin_sequence_number
            && tcp_header.acknowledgement_number() == fin_sequence_number + 1
        {
            cx_debug!(target: TAG, self.id, "Received ACK of FIN");
            self.handle_fin_ack();
        }
    }

    fn handle_first_packet(&mut self, ipv4_packet: &Ipv4Packet) {
        cx_debug!(target: TAG, self.id, "handle_first_packet()");
        let tcp_header = Self::tcp_header_of_packet(ipv4_packet);
        if tcp_header.is_syn() {
            let their_sequence_number = tcp_header.sequence_number();
            self.tcb.acknowledgement_number = Wrapping(their_sequence_number) + Wrapping(1);
            self.tcb.syn_sequence_number = their_sequence_number;

            self.tcb.sequence_number = Wrapping(random::<u32>());
            cx_debug!(
                target: TAG,
                self.id,
                "Initialized seq={}; ack={}",
                self.tcb.sequence_number,
                self.tcb.acknowledgement_number
            );
            self.tcb.client_window = tcp_header.window();
            self.tcb.state = TcpState::SynSent;
            cx_debug!(target: TAG, self.id, "State = {:?}", self.tcb.state);
        } else {
            cx_warn!(
                target: TAG,
                self.id,
                "Unexpected first packet {}; acking {}; flags={}",
                tcp_header.sequence_number(),
                tcp_header.acknowledgement_number(),
                tcp_header.flags()
            );
            // make a RST in the window client
            self.tcb.sequence_number = Wrapping(tcp_header.acknowledgement_number());
            self.send_empty_packet_to_client(tcp_header::FLAG_RST);
            self.close();
        }
    }

    fn handle_duplicate_syn(&mut self, ipv4_packet: &Ipv4Packet) {
        let tcp_header = Self::tcp_header_of_packet(ipv4_packet);
        let their_sequence_number = tcp_header.sequence_number();
        if self.tcb.state == TcpState::SynSent {
            // the connection is not established yet, we can accept this packet as if it were the
            // first SYN
            self.tcb.syn_sequence_number = their_sequence_number;
            self.tcb.acknowledgement_number = Wrapping(their_sequence_number) + Wrapping(1);
        } else if their_sequence_number != self.tcb.syn_sequence_number {
            // duplicate SYN with different sequence number
            self.send_empty_packet_to_client(tcp_header::FLAG_RST);
            self.close();
        }
    }

    fn handle_fin(&mut self) {
        cx_debug!(
            target: TAG,
            self.id,
            "Received a FIN from the client {}",
            self.tcb.numbers()
        );

        self.tcb.fin_received = true;
        if self.client_to_network.is_empty() {
            cx_debug!(
                target: TAG,
                self.id,
                "No pending data, process the FIN immediately"
            );
            self.do_handle_fin();
        }
        // otherwise, the FIN will be processed once client_to_network is empty
    }

    fn do_handle_fin(&mut self) {
        self.tcb.acknowledgement_number += Wrapping(1); // received FIN counts for 1 byte

        if self.tcb.state == TcpState::Established {
            self.send_empty_packet_to_client(tcp_header::FLAG_FIN | tcp_header::FLAG_ACK);
            self.tcb.fin_sequence_number = Some(self.tcb.sequence_number.0);
            self.tcb.sequence_number += Wrapping(1); // FIN counts for 1 byte
            // the connection will be closed by RAII, so switch immediately to LastAck
            // (bypass CloseWait)
            self.tcb.state = TcpState::LastAck;
            cx_debug!(target: TAG, self.id, "State = {:?}", self.tcb.state);
        } else if self.tcb.state == TcpState::FinWait1 {
            self.send_empty_packet_to_client(tcp_header::FLAG_ACK);
            self.tcb.state = TcpState::Closing;
            cx_debug!(target: TAG, self.id, "State = {:?}", self.tcb.state);
        } else if self.tcb.state == TcpState::FinWait2 {
            self.send_empty_packet_to_client(tcp_header::FLAG_ACK);
            self.close();
        } else {
            cx_warn!(
                target: TAG,
                self.id,
                "Received FIN was state was {:?}",
                self.tcb.state
            );
        }
    }

    fn handle_fin_ack(&mut self) {
        if self.tcb.state == TcpState::LastAck || self.tcb.state == TcpState::Closing {
            self.close();
        } else if self.tcb.state == TcpState::FinWait1 {
            self.tcb.state = TcpState::FinWait2;
            cx_debug!(target: TAG, self.id, "State = {:?}", self.tcb.state);
        } else if self.tcb.state != TcpState::FinWait2 {
            cx_warn!(
                target: TAG,
                self.id,
                "Received FIN ACK while state was {:?}",
                self.tcb.state
            );
        }
    }

    fn handle_ack(&mut self, ipv4_packet: &Ipv4Packet) {
        cx_debug!(target: TAG, self.id, "handle_ack()");
        if self.tcb.state == TcpState::SynReceived {
            self.tcb.state = TcpState::Established;
            cx_debug!(target: TAG, self.id, "State = {:?}", self.tcb.state);
            return;
        }

        if tracing::enabled!(target: TAG, Level::TRACE) {
            cx_trace!(
                target: TAG,
                self.id,
                "{}",
                binary::build_packet_string(ipv4_packet.raw())
            );
        }

        let payload = ipv4_packet.payload().expect("No payload");
        if payload.is_empty() {
            // no data to transmit
            return;
        }

        if self.client_to_network.remaining() < payload.len() {
            cx_warn!(target: TAG, self.id, "Not enough space, dropping packet");
            return;
        }

        self.client_to_network.read_from(payload);
        // data will be ACKed once written to the network socket
    }

    fn create_empty_response_packet<'a>(
        id: &ConnectionId,
        packetizer: &'a mut Packetizer,
        tcb: &Tcb,
        flags: u16,
    ) -> Ipv4Packet<'a> {
        Self::update_headers(packetizer.transport_header_mut(), tcb, flags);
        cx_debug!(
            target: TAG,
            id,
            "Forging empty response (flags={}) {}",
            flags,
            tcb.numbers()
        );
        if (flags & tcp_header::FLAG_ACK) != 0 {
            cx_debug!(target: TAG, id, "Acking {}", tcb.numbers());
        }
        let ipv4_packet = packetizer.packetize_empty_payload();
        if tracing::enabled!(target: TAG, Level::TRACE) {
            cx_trace!(
                target: TAG,
                id,
                "{}",
                binary::build_packet_string(ipv4_packet.raw())
            );
        }
        ipv4_packet
    }

    fn may_read(&self) -> bool {
        if !self.tcb.state.is_connected() || self.tcb.state.is_closed() {
            return false;
        }
        if !self.outbox.is_empty() {
            // wait for the client queue
            return false;
        }
        self.tcb.remaining_client_window() > 0
    }

    fn may_write(&self) -> bool {
        !self.client_to_network.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{
        DEVICE_IP, HOST_ALIAS_IP, TcpSegment, connection_id, next_packet, within,
    };
    use std::net::{Ipv4Addr, SocketAddrV4};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;
    use tokio::sync::mpsc;
    use tokio::sync::mpsc::error::TryRecvError;

    const SYN: u16 = tcp_header::FLAG_SYN;
    const ACK: u16 = tcp_header::FLAG_ACK;
    const FIN: u16 = tcp_header::FLAG_FIN;
    const RST: u16 = tcp_header::FLAG_RST;
    const PSH: u16 = tcp_header::FLAG_PSH;

    const WINDOW: u16 = 0xFFFF;
    const CLIENT_ISN: u32 = 1000;

    fn device() -> SocketAddrV4 {
        SocketAddrV4::new(DEVICE_IP, 40000)
    }

    fn remote() -> SocketAddrV4 {
        SocketAddrV4::new(Ipv4Addr::new(93, 184, 216, 34), 80)
    }

    fn from_device(seq: u32, ack: u32, flags: u16, payload: &[u8]) -> Vec<u8> {
        from_device_to(remote(), seq, ack, flags, WINDOW, payload)
    }

    fn from_device_to(
        destination: SocketAddrV4,
        seq: u32,
        ack: u32,
        flags: u16,
        window: u16,
        payload: &[u8],
    ) -> Vec<u8> {
        TcpSegment {
            source: device(),
            destination,
            seq,
            ack,
            flags,
            window,
            payload: payload.to_vec(),
        }
        .to_packet()
    }

    /// Create a connection from its first packet, as `run()` does, without connecting
    fn open_with(mut first_packet: Vec<u8>) -> (TcpConnection, mpsc::Receiver<Vec<u8>>) {
        let (sender, receiver) = mpsc::channel(16);
        let id = connection_id(&mut first_packet);
        let mut connection = TcpConnection::new(id, sender, &mut first_packet);
        connection.handle_raw_packet(&mut first_packet);
        (connection, receiver)
    }

    fn open() -> (TcpConnection, mpsc::Receiver<Vec<u8>>) {
        let (mut connection, receiver) = open_with(from_device(CLIENT_ISN, 0, SYN, &[]));
        // the SYN-ACK takes the last sequence number, so that the next ones wrap around
        connection.tcb.sequence_number = Wrapping(u32::MAX);
        (connection, receiver)
    }

    /// Complete the handshake, return the next sequence number of the relay
    fn establish(connection: &mut TcpConnection, receiver: &mut mpsc::Receiver<Vec<u8>>) -> u32 {
        connection.process_connect();
        let syn_ack = sent(receiver);
        let seq = syn_ack.seq.wrapping_add(1);
        connection.handle_raw_packet(&mut from_device(CLIENT_ISN + 1, seq, ACK, &[]));
        assert_eq!(TcpState::Established, connection.tcb.state);
        seq
    }

    fn sent(receiver: &mut mpsc::Receiver<Vec<u8>>) -> TcpSegment {
        let segment = TcpSegment::parse(&receiver.try_recv().expect("No packet sent"));
        assert_eq!(remote(), segment.source);
        assert_eq!(device(), segment.destination);
        segment
    }

    fn assert_nothing_sent(receiver: &mut mpsc::Receiver<Vec<u8>>) {
        assert_eq!(Err(TryRecvError::Empty), receiver.try_recv());
    }

    #[test]
    fn answer_syn_once_connected() {
        let (mut connection, mut receiver) = open();
        assert_eq!(TcpState::SynSent, connection.tcb.state);
        // the SYN is acknowledged only once the destination accepted the connection
        assert_nothing_sent(&mut receiver);

        connection.process_connect();
        let syn_ack = sent(&mut receiver);
        assert_eq!(SYN | ACK, syn_ack.flags);
        assert_eq!(u32::MAX, syn_ack.seq);
        assert_eq!(CLIENT_ISN + 1, syn_ack.ack);
        assert!(syn_ack.payload.is_empty());
        assert_eq!(TcpState::SynReceived, connection.tcb.state);

        connection.handle_raw_packet(&mut from_device(CLIENT_ISN + 1, 0, ACK, &[]));
        assert_eq!(TcpState::Established, connection.tcb.state);
        assert!(!connection.closed);
    }

    #[test]
    fn reset_unexpected_first_packet() {
        let (connection, mut receiver) = open_with(from_device(5000, 7000, ACK, b"data"));
        let rst = sent(&mut receiver);
        assert_eq!(RST, rst.flags);
        // the RST must fall in the window of the client
        assert_eq!(7000, rst.seq);
        assert!(connection.closed);
    }

    #[test]
    fn duplicate_syn_before_connection_replaces_sequence_number() {
        let (mut connection, mut receiver) = open();
        connection.handle_raw_packet(&mut from_device(2000, 0, SYN, &[]));
        connection.process_connect();
        assert_eq!(2001, sent(&mut receiver).ack);
    }

    #[test]
    fn duplicate_syn_after_connection() {
        let (mut connection, mut receiver) = open();
        connection.process_connect();
        sent(&mut receiver);

        // retransmission of the same SYN
        connection.handle_raw_packet(&mut from_device(CLIENT_ISN, 0, SYN, &[]));
        assert_nothing_sent(&mut receiver);
        assert!(!connection.closed);

        // SYN of another connection
        connection.handle_raw_packet(&mut from_device(3000, 0, SYN, &[]));
        assert_eq!(RST, sent(&mut receiver).flags);
        assert!(connection.closed);
    }

    #[test]
    fn buffer_client_data_until_written() {
        let (mut connection, mut receiver) = open();
        let seq = establish(&mut connection, &mut receiver);

        connection.handle_raw_packet(&mut from_device(CLIENT_ISN + 1, seq, ACK | PSH, b"abc"));
        // the next segment starts after the data not written yet
        connection.handle_raw_packet(&mut from_device(CLIENT_ISN + 4, seq, ACK | PSH, b"de"));

        assert_eq!(5, connection.client_to_network.size());
        assert!(connection.may_write());
        // data is acknowledged once written to the network
        assert_nothing_sent(&mut receiver);
    }

    #[test]
    fn acknowledge_unexpected_segment() {
        let (mut connection, mut receiver) = open();
        let seq = establish(&mut connection, &mut receiver);
        connection.handle_raw_packet(&mut from_device(CLIENT_ISN + 1, seq, ACK | PSH, b"abc"));

        // retransmission of data already received
        connection.handle_raw_packet(&mut from_device(CLIENT_ISN + 1, seq, ACK | PSH, b"abc"));
        let ack = sent(&mut receiver);
        assert_eq!(ACK, ack.flags);
        assert_eq!(seq, ack.seq);
        assert_eq!(CLIENT_ISN + 1, ack.ack);
        assert_eq!(3, connection.client_to_network.size());

        // keepalive probe: one byte before the last acknowledged sequence number
        connection.handle_raw_packet(&mut from_device(CLIENT_ISN, seq, ACK, &[]));
        let ack = sent(&mut receiver);
        assert_eq!(ACK, ack.flags);
        assert_eq!(seq, ack.seq);
        assert_eq!(CLIENT_ISN + 1, ack.ack);
        assert_eq!(3, connection.client_to_network.size());
        assert!(!connection.closed);
    }

    #[test]
    fn ignore_unexpected_rst() {
        let (mut connection, mut receiver) = open();
        let seq = establish(&mut connection, &mut receiver);
        connection.handle_raw_packet(&mut from_device(CLIENT_ISN + 100, seq, RST, &[]));
        assert_nothing_sent(&mut receiver);
        assert!(!connection.closed);
    }

    #[test]
    fn close_on_rst() {
        let (mut connection, mut receiver) = open();
        let seq = establish(&mut connection, &mut receiver);
        connection.handle_raw_packet(&mut from_device(CLIENT_ISN + 1, seq, RST, &[]));
        assert_nothing_sent(&mut receiver);
        assert!(connection.closed);
    }

    #[test]
    fn close_initiated_by_client() {
        let (mut connection, mut receiver) = open();
        let seq = establish(&mut connection, &mut receiver);

        connection.handle_raw_packet(&mut from_device(CLIENT_ISN + 1, seq, FIN | ACK, &[]));
        let fin = sent(&mut receiver);
        assert_eq!(FIN | ACK, fin.flags);
        assert_eq!(seq, fin.seq);
        assert_eq!(CLIENT_ISN + 2, fin.ack);
        assert_eq!(TcpState::LastAck, connection.tcb.state);
        assert!(!connection.closed);

        connection.handle_raw_packet(&mut from_device(CLIENT_ISN + 2, seq + 1, ACK, &[]));
        assert!(connection.closed);
    }

    #[tokio::test]
    async fn defer_client_fin_until_data_is_written() {
        let (mut connection, mut receiver) = open();
        let seq = establish(&mut connection, &mut receiver);

        connection.handle_raw_packet(&mut from_device(CLIENT_ISN + 1, seq, ACK | PSH, b"abc"));
        connection.handle_raw_packet(&mut from_device(CLIENT_ISN + 4, seq, FIN | ACK, &[]));
        assert!(connection.tcb.fin_received);
        assert_eq!(TcpState::Established, connection.tcb.state);
        assert_nothing_sent(&mut receiver);

        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
        let stream = TcpStream::connect(listener.local_addr().unwrap())
            .await
            .unwrap();
        let (mut server, _) = listener.accept().await.unwrap();
        within(stream.writable()).await.unwrap();
        connection.process_send(&stream);

        // the FIN is acknowledged along with the data
        let fin = sent(&mut receiver);
        assert_eq!(FIN | ACK, fin.flags);
        assert_eq!(seq, fin.seq);
        assert_eq!(CLIENT_ISN + 5, fin.ack);
        assert_nothing_sent(&mut receiver);
        assert_eq!(TcpState::LastAck, connection.tcb.state);
        let mut buf = [0u8; 3];
        within(server.read_exact(&mut buf)).await.unwrap();
        assert_eq!(b"abc", &buf);
    }

    #[test]
    fn close_initiated_by_network() {
        let (mut connection, mut receiver) = open();
        let seq = establish(&mut connection, &mut receiver);

        connection.eof();
        let fin = sent(&mut receiver);
        assert_eq!(FIN | ACK, fin.flags);
        assert_eq!(seq, fin.seq);
        assert_eq!(TcpState::FinWait1, connection.tcb.state);
        assert!(!connection.may_read());

        connection.handle_raw_packet(&mut from_device(CLIENT_ISN + 1, seq + 1, ACK, &[]));
        assert_eq!(TcpState::FinWait2, connection.tcb.state);
        assert_nothing_sent(&mut receiver);

        connection.handle_raw_packet(&mut from_device(CLIENT_ISN + 1, seq + 1, FIN | ACK, &[]));
        let ack = sent(&mut receiver);
        assert_eq!(ACK, ack.flags);
        assert_eq!(CLIENT_ISN + 2, ack.ack);
        assert!(connection.closed);
    }

    #[test]
    fn simultaneous_close() {
        let (mut connection, mut receiver) = open();
        let seq = establish(&mut connection, &mut receiver);

        connection.eof();
        sent(&mut receiver);
        // the client sends its FIN before receiving ours
        connection.handle_raw_packet(&mut from_device(CLIENT_ISN + 1, seq, FIN | ACK, &[]));
        assert_eq!(ACK, sent(&mut receiver).flags);
        assert_eq!(TcpState::Closing, connection.tcb.state);
        assert!(!connection.closed);

        connection.handle_raw_packet(&mut from_device(CLIENT_ISN + 2, seq + 1, ACK, &[]));
        assert!(connection.closed);
    }

    #[test]
    fn do_not_read_when_client_window_is_full() {
        let (mut connection, mut receiver) = open();
        let seq = establish(&mut connection, &mut receiver);
        assert!(connection.may_read());

        connection.handle_raw_packet(&mut from_device_to(
            remote(),
            CLIENT_ISN + 1,
            seq,
            ACK,
            0,
            &[],
        ));
        assert!(!connection.may_read());
    }

    #[test]
    fn remaining_client_window() {
        let mut tcb = Tcb::new();
        tcb.their_acknowledgement_number = 100;
        tcb.client_window = 50;

        tcb.sequence_number = Wrapping(120);
        assert_eq!(30, tcb.remaining_client_window());
        tcb.sequence_number = Wrapping(150);
        assert_eq!(0, tcb.remaining_client_window());
        // the client shrank its window below the data already sent
        tcb.sequence_number = Wrapping(160);
        assert_eq!(0, tcb.remaining_client_window());

        // the sequence numbers wrapped around
        tcb.their_acknowledgement_number = u32::MAX - 9;
        tcb.sequence_number = Wrapping(5);
        assert_eq!(35, tcb.remaining_client_window());
    }

    /// Relay a TCP connection to a local server, through the host alias of the Android emulator
    struct Relayed {
        server: TcpStream,
        to_relay: mpsc::Sender<Vec<u8>>,
        from_relay: mpsc::Receiver<Vec<u8>>,
        relay: tokio::task::JoinHandle<()>,
        destination: SocketAddrV4,
        // next sequence number of the relay
        seq: u32,
    }

    impl Relayed {
        async fn connect() -> Self {
            let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
            let destination =
                SocketAddrV4::new(HOST_ALIAS_IP, listener.local_addr().unwrap().port());
            let (to_client, mut from_relay) = mpsc::channel(64);
            let (to_relay, inbound) = mpsc::channel(64);

            let mut syn = from_device_to(destination, CLIENT_ISN, 0, SYN, WINDOW, &[]);
            let id = connection_id(&mut syn);
            let relay = tokio::spawn(TcpConnection::run(id, to_client, syn, inbound));
            let (server, _) = within(listener.accept()).await.unwrap();

            let syn_ack = TcpSegment::parse(&next_packet(&mut from_relay).await);
            assert_eq!(SYN | ACK, syn_ack.flags);
            assert_eq!(destination, syn_ack.source);
            assert_eq!(device(), syn_ack.destination);
            assert_eq!(CLIENT_ISN + 1, syn_ack.ack);

            let relayed = Self {
                server,
                to_relay,
                from_relay,
                relay,
                destination,
                seq: syn_ack.seq.wrapping_add(1),
            };
            relayed.send(CLIENT_ISN + 1, ACK, &[]).await;
            relayed
        }

        async fn send(&self, seq: u32, flags: u16, payload: &[u8]) {
            let packet = from_device_to(self.destination, seq, self.seq, flags, WINDOW, payload);
            self.to_relay.send(packet).await.unwrap();
        }

        async fn receive(&mut self) -> TcpSegment {
            let segment = TcpSegment::parse(&next_packet(&mut self.from_relay).await);
            assert_eq!(self.destination, segment.source);
            assert_eq!(device(), segment.destination);
            segment
        }
    }

    #[tokio::test]
    async fn relay_data_both_ways_until_network_closes() {
        let mut relayed = Relayed::connect().await;

        relayed.send(CLIENT_ISN + 1, ACK | PSH, b"hello").await;
        let mut buf = [0u8; 5];
        within(relayed.server.read_exact(&mut buf)).await.unwrap();
        assert_eq!(b"hello", &buf);
        let ack = relayed.receive().await;
        assert_eq!(ACK, ack.flags);
        assert_eq!(CLIENT_ISN + 6, ack.ack);

        relayed.server.write_all(b"world").await.unwrap();
        let data = relayed.receive().await;
        assert_eq!(ACK | PSH, data.flags);
        assert_eq!(relayed.seq, data.seq);
        assert_eq!(b"world", &data.payload[..]);
        relayed.seq = relayed.seq.wrapping_add(5);

        relayed.server.shutdown().await.unwrap();
        let fin = relayed.receive().await;
        assert_eq!(FIN | ACK, fin.flags);
        assert_eq!(relayed.seq, fin.seq);
        relayed.seq = relayed.seq.wrapping_add(1);

        relayed.send(CLIENT_ISN + 6, FIN | ACK, &[]).await;
        let last_ack = relayed.receive().await;
        assert_eq!(ACK, last_ack.flags);
        assert_eq!(CLIENT_ISN + 7, last_ack.ack);
        within(relayed.relay).await.unwrap();
    }

    #[tokio::test]
    async fn deliver_data_before_closing_on_client_fin() {
        let mut relayed = Relayed::connect().await;

        relayed.send(CLIENT_ISN + 1, ACK | PSH, b"bye").await;
        relayed.send(CLIENT_ISN + 4, FIN | ACK, &[]).await;

        // the data may be acknowledged before the FIN arrives, the FIN is acknowledged after it
        let fin = loop {
            let segment = relayed.receive().await;
            if segment.flags != ACK {
                break segment;
            }
            assert_eq!(CLIENT_ISN + 4, segment.ack);
        };
        assert_eq!(FIN | ACK, fin.flags);
        assert_eq!(relayed.seq, fin.seq);
        assert_eq!(CLIENT_ISN + 5, fin.ack);
        relayed.seq = relayed.seq.wrapping_add(1);
        relayed.send(CLIENT_ISN + 5, ACK, &[]).await;
        within(relayed.relay).await.unwrap();

        let mut received = Vec::new();
        within(relayed.server.read_to_end(&mut received))
            .await
            .unwrap();
        assert_eq!(b"bye", &received[..]);
    }
}
