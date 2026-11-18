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

use std::io;
use std::net::{self, Ipv4Addr, SocketAddr};
use std::time::Duration;
use tokio::net::UdpSocket;
use tokio::sync::mpsc::Receiver;
use tokio::task::coop;
use tokio::time::{self, Instant};
use tracing::Level;

use super::id::ConnectionId;
use super::outbox::Outbox;
use crate::binary;
use crate::packet::ipv4_packet::{Ipv4Packet, MAX_PACKET_LENGTH};
use crate::packet::packetizer::Packetizer;
use crate::server::ClientSender;

const TAG: &str = "UdpConnection";

const IDLE_TIMEOUT: Duration = Duration::from_secs(2 * 60);

/// Maximum size of the datagrams waiting for the client queue
///
/// Beyond it, datagrams wait in the socket buffer (and the system drops them when it is full).
const MAX_BACKLOG: usize = 16 * MAX_PACKET_LENGTH;

pub struct UdpConnection {
    id: ConnectionId,
    // packets for the client, sent in order
    outbox: Outbox,
    network_to_client: Packetizer,
}

enum Event {
    Packet(Option<Vec<u8>>),
    Readable(io::Result<()>),
    // false if the client is closed
    Flushed(bool),
    Expired,
}

impl UdpConnection {
    /// Relay datagrams between the client and the destination, until the connection is idle
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
        match Self::create_socket() {
            Ok(socket) => {
                if Self::send_to_network(&connection.id, &socket, first_packet).await {
                    connection.process(&socket, &mut inbound).await;
                }
            }
            Err(err) => cx_error!(target: TAG, connection.id, "Cannot open socket: {}", err),
        }
        cx_info!(target: TAG, connection.id, "Close");
    }

    fn new(id: ConnectionId, client: ClientSender, first_packet: &mut [u8]) -> Self {
        let ipv4_packet = Ipv4Packet::parse(first_packet);
        let (ipv4_header, transport_header) = ipv4_packet.headers();
        let packetizer = Packetizer::new(&ipv4_header, &transport_header.expect("No transport"));
        Self {
            id,
            outbox: Outbox::new(client),
            network_to_client: packetizer,
        }
    }

    /// Create the socket of a flow
    ///
    /// It is not connected: replies may come from another port of the contacted host, or from
    /// any host for a broadcast (see [`ConnectionId::accepts_reply_from`]). Broadcast is allowed
    /// so the client can also send to subnet broadcast addresses (e.g. 192.168.1.255); only
    /// 255.255.255.255 counts as broadcast for replies, so replies to a subnet broadcast are
    /// dropped unless they come from that address.
    fn create_socket() -> io::Result<UdpSocket> {
        let socket = net::UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0))?;
        socket.set_broadcast(true)?;
        socket.set_nonblocking(true)?;
        UdpSocket::from_std(socket)
    }

    async fn process(&mut self, socket: &UdpSocket, inbound: &mut Receiver<Vec<u8>>) {
        let idle = time::sleep(IDLE_TIMEOUT);
        tokio::pin!(idle);
        loop {
            let may_read = self.outbox.bytes() < MAX_BACKLOG;
            let has_backlog = !self.outbox.is_empty();
            let event = tokio::select! {
                packet = inbound.recv() => Event::Packet(packet),
                result = socket.readable(), if may_read => Event::Readable(result),
                open = self.outbox.flush(), if has_backlog => Event::Flushed(open),
                _ = &mut idle => Event::Expired,
            };
            let open = match event {
                Event::Packet(Some(raw)) => Self::send_to_network(&self.id, socket, raw).await,
                // the client is closed
                Event::Packet(None) => false,
                Event::Readable(Ok(())) => {
                    let received = self.receive(socket);
                    // readiness does not consume the task budget, let the other tasks run
                    coop::consume_budget().await;
                    match received {
                        Some(open) => open,
                        // nothing relayed: not activity, keep the idle deadline
                        None => continue,
                    }
                }
                Event::Readable(Err(err)) => {
                    cx_error!(target: TAG, self.id, "Socket error: {}", err);
                    false
                }
                Event::Flushed(open) => open,
                Event::Expired => {
                    cx_debug!(target: TAG, self.id, "Idle for too long");
                    false
                }
            };
            if !open {
                return;
            }
            idle.as_mut().reset(Instant::now() + IDLE_TIMEOUT);
        }
    }

    /// Send the payload of a packet from the client, return false if the connection must close
    async fn send_to_network(id: &ConnectionId, socket: &UdpSocket, mut raw: Vec<u8>) -> bool {
        let ipv4_packet = Ipv4Packet::parse(&mut raw);
        let payload = ipv4_packet.payload().expect("No payload");
        match socket.send_to(payload, id.rewritten_destination()).await {
            Ok(_) => true,
            Err(err) => {
                cx_error!(target: TAG, id, "Cannot write: [{:?}] {}", err.kind(), err);
                false
            }
        }
    }

    /// Forward a datagram to the client
    ///
    /// Return `Some(false)` if the connection must close, `Some(true)` if a datagram was relayed,
    /// or `None` if nothing was (a datagram this flow does not accept, or no datagram yet).
    fn receive(&mut self, socket: &UdpSocket) -> Option<bool> {
        let id = &self.id;
        let received = self.network_to_client.packetize_from(|buf| {
            let (length, sender) = socket.try_recv_from(buf)?;
            let SocketAddr::V4(sender) = sender else {
                return Ok(None);
            };
            if !id.accepts_reply_from(sender) {
                cx_debug!(target: TAG, id, "Dropped datagram from {}", sender);
                return Ok(None);
            }
            Ok(Some((length, ConnectionId::client_visible_source(sender))))
        });
        let ipv4_packet = match received {
            Ok(Some(ipv4_packet)) => ipv4_packet,
            // not for this flow, keep it open
            Ok(None) => return None,
            Err(err) if err.kind() == io::ErrorKind::WouldBlock => return None,
            Err(err) => {
                cx_error!(target: TAG, self.id, "Cannot read: [{:?}] {}", err.kind(), err);
                return Some(false);
            }
        };
        if tracing::enabled!(target: TAG, Level::TRACE) {
            cx_trace!(
                target: TAG,
                self.id,
                "{}",
                binary::build_packet_string(ipv4_packet.raw())
            );
        }
        let length = ipv4_packet.length();
        let open = self.outbox.push(ipv4_packet.raw().to_vec());
        cx_debug!(
            target: TAG,
            self.id,
            "Packet ({} bytes) sent to client",
            length
        );
        Some(open)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{
        DEVICE_IP, HOST_ALIAS_IP, UdpDatagram, connection_id, next_packet, within,
    };
    use std::net::SocketAddrV4;
    use tokio::sync::mpsc;

    #[tokio::test]
    async fn relay_datagrams_both_ways_until_client_closes() {
        let server = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
        let destination = SocketAddrV4::new(HOST_ALIAS_IP, server.local_addr().unwrap().port());
        let device = SocketAddrV4::new(DEVICE_IP, 50000);
        let datagram = |payload: &[u8]| {
            UdpDatagram {
                source: device,
                destination,
                payload: payload.to_vec(),
            }
            .to_packet()
        };

        let (to_client, mut from_relay) = mpsc::channel(16);
        let (to_relay, inbound) = mpsc::channel(16);
        let mut first_packet = datagram(b"ping");
        let id = connection_id(&mut first_packet);
        let relay = tokio::spawn(UdpConnection::run(id, to_client, first_packet, inbound));

        let mut buf = [0u8; 64];
        let (len, peer) = within(server.recv_from(&mut buf)).await.unwrap();
        assert_eq!(b"ping", &buf[..len]);

        server.send_to(b"pong", peer).await.unwrap();
        let response = UdpDatagram::parse(&next_packet(&mut from_relay).await);
        assert_eq!(
            UdpDatagram {
                source: destination,
                destination: device,
                payload: b"pong".to_vec(),
            },
            response
        );

        // the next datagrams use the same socket
        to_relay.send(datagram(b"again")).await.unwrap();
        let (len, next_peer) = within(server.recv_from(&mut buf)).await.unwrap();
        assert_eq!(b"again", &buf[..len]);
        assert_eq!(peer, next_peer);

        drop(to_relay);
        within(relay).await.unwrap();
    }

    // 127.0.0.2 is only routed to loopback by default on Linux
    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn drop_datagrams_from_other_hosts_and_keep_relaying() {
        let server = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
        let stranger = UdpSocket::bind((Ipv4Addr::new(127, 0, 0, 2), 0))
            .await
            .unwrap();
        let destination = SocketAddrV4::new(HOST_ALIAS_IP, server.local_addr().unwrap().port());
        let device = SocketAddrV4::new(DEVICE_IP, 50002);
        let mut first_packet = UdpDatagram {
            source: device,
            destination,
            payload: b"hello".to_vec(),
        }
        .to_packet();

        let (to_client, mut from_relay) = mpsc::channel(16);
        let (to_relay, inbound) = mpsc::channel(16);
        let id = connection_id(&mut first_packet);
        let relay = tokio::spawn(UdpConnection::run(id, to_client, first_packet, inbound));

        let mut buf = [0u8; 64];
        let (_, peer) = within(server.recv_from(&mut buf)).await.unwrap();
        stranger.send_to(b"intruder", peer).await.unwrap();
        server.send_to(b"reply", peer).await.unwrap();

        // the stranger's datagram never reaches the client, and the flow is still open
        let response = UdpDatagram::parse(&next_packet(&mut from_relay).await);
        assert_eq!(b"reply".to_vec(), response.payload);
        assert!(!relay.is_finished());

        drop(to_relay);
        within(relay).await.unwrap();
    }

    #[tokio::test]
    async fn relay_replies_from_another_port_of_the_contacted_host() {
        let server = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
        let other = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
        let destination = SocketAddrV4::new(HOST_ALIAS_IP, server.local_addr().unwrap().port());
        let device = SocketAddrV4::new(DEVICE_IP, 50001);
        let mut first_packet = UdpDatagram {
            source: device,
            destination,
            payload: b"hello".to_vec(),
        }
        .to_packet();

        let (to_client, mut from_relay) = mpsc::channel(16);
        let (to_relay, inbound) = mpsc::channel(16);
        let id = connection_id(&mut first_packet);
        let relay = tokio::spawn(UdpConnection::run(id, to_client, first_packet, inbound));

        let mut buf = [0u8; 64];
        let (_, peer) = within(server.recv_from(&mut buf)).await.unwrap();
        // answer from a socket the client never contacted
        other.send_to(b"from other port", peer).await.unwrap();
        let response = UdpDatagram::parse(&next_packet(&mut from_relay).await);
        let other_port = other.local_addr().unwrap().port();
        assert_eq!(
            UdpDatagram {
                // shown through the host alias, with the real sender port
                source: SocketAddrV4::new(HOST_ALIAS_IP, other_port),
                destination: device,
                payload: b"from other port".to_vec(),
            },
            response
        );

        drop(to_relay);
        within(relay).await.unwrap();
    }
}
