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

use std::collections::HashMap;
use tokio::sync::mpsc;
use tokio::sync::mpsc::error::TrySendError;
use tracing::{Level, error, trace, warn};

use super::binary;
use super::client::ClientSender;
use super::connection::ConnectionId;
use super::ipv4_header::Protocol;
use super::ipv4_packet::Ipv4Packet;
use super::tcp_connection::TcpConnection;
use super::udp_connection::UdpConnection;

const TAG: &str = "Router";

/// Number of packets from the client queued for a connection
///
/// When the queue is full, packets are dropped (TCP retransmits them).
///
/// A single chunk read from the client may contain more than a hundred packets for the same
/// connection (the client sends 536-byte TCP segments).
const QUEUE_CAPACITY: usize = 512;

/// Dispatch the packets from the client to their connection
pub struct Router {
    client: ClientSender,
    // each connection runs in its own task, until its queue is closed
    connections: HashMap<ConnectionId, mpsc::Sender<Vec<u8>>>,
}

impl Router {
    pub fn new(client: ClientSender) -> Self {
        Self {
            client,
            connections: HashMap::new(),
        }
    }

    pub fn send_to_network(&mut self, ipv4_packet: &Ipv4Packet) {
        if !ipv4_packet.is_valid() {
            warn!(target: TAG, "Dropping invalid packet");
            if tracing::enabled!(target: TAG, Level::TRACE) {
                trace!(
                    target: TAG,
                    "{}",
                    binary::build_packet_string(ipv4_packet.raw())
                );
            }
            return;
        }

        let (ipv4_header_data, transport_header_data) = ipv4_packet.headers_data();
        let transport_header_data = transport_header_data.expect("No transport");
        let id = ConnectionId::from_headers(ipv4_header_data, transport_header_data);

        if let Some(connection) = self.connections.get(&id) {
            match connection.try_send(ipv4_packet.raw().to_vec()) {
                Ok(()) => return,
                Err(TrySendError::Full(_)) => {
                    cx_warn!(target: TAG, id, "Connection busy, dropping packet");
                    return;
                }
                // the connection terminated, the packet belongs to a new one
                Err(TrySendError::Closed(_)) => (),
            }
        }

        // forget the terminated connections
        self.connections
            .retain(|_, connection| !connection.is_closed());

        let (sender, receiver) = mpsc::channel(QUEUE_CAPACITY);
        let packet = ipv4_packet.raw().to_vec();
        let client = self.client.clone();
        match id.protocol() {
            Protocol::Tcp => {
                tokio::spawn(TcpConnection::run(id.clone(), client, packet, receiver));
            }
            Protocol::Udp => {
                tokio::spawn(UdpConnection::run(id.clone(), client, packet, receiver));
            }
            protocol => {
                error!(
                    target: TAG,
                    "Cannot create route, dropping packet: Unsupported protocol: {:?}", protocol
                );
                return;
            }
        }
        self.connections.insert(id, sender);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::relay::tcp_header::FLAG_SYN;
    use crate::relay::test_support::{
        DEVICE_IP, HOST_ALIAS_IP, TcpSegment, UdpDatagram, connection_id, other_protocol_packet,
        within,
    };
    use std::net::{Ipv4Addr, SocketAddrV4};
    use tokio::net::UdpSocket;

    fn route(router: &mut Router, mut packet: Vec<u8>) {
        router.send_to_network(&Ipv4Packet::parse(&mut packet));
    }

    #[tokio::test]
    async fn drop_packets_of_unsupported_protocols() {
        let (client, _receiver) = mpsc::channel(16);
        let mut router = Router::new(client);
        let icmp = other_protocol_packet(
            1,
            SocketAddrV4::new(DEVICE_IP, 0),
            SocketAddrV4::new(HOST_ALIAS_IP, 0),
        );
        route(&mut router, icmp);
        assert!(router.connections.is_empty());
    }

    #[tokio::test]
    async fn route_packets_of_a_flow_to_the_same_connection() {
        let server = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
        let destination = SocketAddrV4::new(HOST_ALIAS_IP, server.local_addr().unwrap().port());
        let datagram = |source_port: u16, payload: &[u8]| {
            UdpDatagram {
                source: SocketAddrV4::new(DEVICE_IP, source_port),
                destination,
                payload: payload.to_vec(),
            }
            .to_packet()
        };
        let (client, _receiver) = mpsc::channel(16);
        let mut router = Router::new(client);
        let mut buf = [0u8; 64];

        route(&mut router, datagram(50000, b"first"));
        let (_, first_peer) = within(server.recv_from(&mut buf)).await.unwrap();
        route(&mut router, datagram(50000, b"second"));
        let (len, second_peer) = within(server.recv_from(&mut buf)).await.unwrap();
        assert_eq!(b"second", &buf[..len]);
        assert_eq!(first_peer, second_peer);

        route(&mut router, datagram(50001, b"other"));
        let (len, other_peer) = within(server.recv_from(&mut buf)).await.unwrap();
        assert_eq!(b"other", &buf[..len]);
        assert_ne!(first_peer, other_peer);
        assert_eq!(2, router.connections.len());
    }

    #[tokio::test]
    async fn open_new_connection_after_previous_one_terminated() {
        // nothing listens on this port: the connection terminates as soon as it is refused
        let port = std::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let syn = || {
            TcpSegment {
                source: SocketAddrV4::new(DEVICE_IP, 40000),
                destination: SocketAddrV4::new(HOST_ALIAS_IP, port),
                seq: 1000,
                ack: 0,
                flags: FLAG_SYN,
                window: 0xFFFF,
                payload: Vec::new(),
            }
            .to_packet()
        };
        let id = connection_id(&mut syn());
        let (client, _receiver) = mpsc::channel(16);
        let mut router = Router::new(client);

        route(&mut router, syn());
        within(async {
            while !router.connections[&id].is_closed() {
                tokio::time::sleep(std::time::Duration::from_millis(1)).await;
            }
        })
        .await;

        route(&mut router, syn());
        assert_eq!(1, router.connections.len());
        assert!(!router.connections[&id].is_closed());
    }
}
