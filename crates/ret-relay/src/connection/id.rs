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

use std::fmt;
use std::net::{Ipv4Addr, SocketAddrV4};

use crate::net;
use crate::packet::ipv4_header::{Ipv4HeaderData, Protocol};
use crate::packet::transport::TransportHeaderData;

const LOCALHOST_FORWARD: u32 = 0x0A_00_02_02; // 10.0.2.2
const LOCALHOST: u32 = 0x7F_00_00_01; // 127.0.0.1

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ConnectionId {
    protocol: Protocol,
    source_ip: u32,
    source_port: u16,
    destination_ip: u32,
    destination_port: u16,
    id_string: String,
}

impl ConnectionId {
    pub fn from_headers(
        ipv4_header_data: &Ipv4HeaderData,
        transport_header_data: &TransportHeaderData,
    ) -> Self {
        let source_ip = ipv4_header_data.source();
        let source_port = transport_header_data.source_port();
        let destination_ip = ipv4_header_data.destination();
        let destination_port = transport_header_data.destination_port();
        let id_string = format!(
            "{} → {}",
            net::to_socket_addr(source_ip, source_port),
            net::to_socket_addr(destination_ip, destination_port)
        );
        Self {
            protocol: ipv4_header_data.protocol(),
            source_ip,
            source_port,
            destination_ip,
            destination_port,
            id_string,
        }
    }

    pub fn protocol(&self) -> Protocol {
        self.protocol
    }

    pub fn rewritten_destination(&self) -> SocketAddrV4 {
        let ip = if self.destination_ip == LOCALHOST_FORWARD {
            LOCALHOST
        } else {
            self.destination_ip
        };
        net::to_socket_addr(ip, self.destination_port)
    }

    /// Whether the client sent this flow to the limited broadcast address (255.255.255.255)
    pub fn is_broadcast(&self) -> bool {
        self.destination_ip == u32::from(Ipv4Addr::BROADCAST)
    }

    /// Whether a datagram received from `sender` belongs to this flow
    ///
    /// Broadcast flows accept any sender (the replies come from whoever answered). Other flows
    /// accept any port of the contacted host, so a server may answer from another socket.
    pub fn accepts_reply_from(&self, sender: SocketAddrV4) -> bool {
        self.is_broadcast() || sender.ip() == self.rewritten_destination().ip()
    }

    /// The address the client must see as the source of a datagram received from `sender`
    ///
    /// The client reaches the computer through the 10.0.2.2 alias, never through its own
    /// loopback, so loopback senders are shown as the alias.
    pub fn client_visible_source(sender: SocketAddrV4) -> SocketAddrV4 {
        if sender.ip().is_loopback() {
            net::to_socket_addr(LOCALHOST_FORWARD, sender.port())
        } else {
            sender
        }
    }
}

impl fmt::Display for ConnectionId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.id_string)
    }
}

#[cfg(test)]
mod tests {
    use crate::test_support::{DEVICE_IP, HOST_ALIAS_IP, UdpDatagram, connection_id};
    use std::net::{Ipv4Addr, SocketAddrV4};

    fn id_to(destination: SocketAddrV4) -> super::ConnectionId {
        let mut packet = UdpDatagram {
            source: SocketAddrV4::new(DEVICE_IP, 50000),
            destination,
            payload: Vec::new(),
        }
        .to_packet();
        connection_id(&mut packet)
    }

    #[test]
    fn rewrite_host_alias_to_localhost() {
        let id = id_to(SocketAddrV4::new(HOST_ALIAS_IP, 8080));
        assert_eq!(
            SocketAddrV4::new(Ipv4Addr::LOCALHOST, 8080),
            id.rewritten_destination()
        );
    }

    #[test]
    fn keep_other_destinations() {
        let destination = SocketAddrV4::new(Ipv4Addr::new(10, 0, 2, 3), 53);
        assert_eq!(destination, id_to(destination).rewritten_destination());
    }

    #[test]
    fn detect_limited_broadcast() {
        assert!(id_to(SocketAddrV4::new(Ipv4Addr::BROADCAST, 27036)).is_broadcast());
        assert!(!id_to(SocketAddrV4::new(Ipv4Addr::new(192, 168, 1, 255), 27036)).is_broadcast());
    }

    #[test]
    fn broadcast_accepts_any_sender() {
        let id = id_to(SocketAddrV4::new(Ipv4Addr::BROADCAST, 27036));
        assert!(id.accepts_reply_from(SocketAddrV4::new(Ipv4Addr::new(192, 168, 1, 20), 27036)));
        assert!(id.accepts_reply_from(SocketAddrV4::new(Ipv4Addr::new(10, 9, 8, 7), 1234)));
    }

    #[test]
    fn unicast_accepts_any_port_of_the_contacted_host_only() {
        let id = id_to(SocketAddrV4::new(Ipv4Addr::new(192, 168, 1, 20), 27036));
        assert!(id.accepts_reply_from(SocketAddrV4::new(Ipv4Addr::new(192, 168, 1, 20), 27036)));
        assert!(id.accepts_reply_from(SocketAddrV4::new(Ipv4Addr::new(192, 168, 1, 20), 40000)));
        assert!(!id.accepts_reply_from(SocketAddrV4::new(Ipv4Addr::new(192, 168, 1, 21), 27036)));
    }

    #[test]
    fn host_alias_flow_accepts_loopback_replies() {
        let id = id_to(SocketAddrV4::new(HOST_ALIAS_IP, 8080));
        assert!(id.accepts_reply_from(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 9090)));
    }

    #[test]
    fn show_loopback_senders_as_host_alias() {
        assert_eq!(
            SocketAddrV4::new(HOST_ALIAS_IP, 9090),
            super::ConnectionId::client_visible_source(SocketAddrV4::new(
                Ipv4Addr::LOCALHOST,
                9090
            ))
        );
        let lan = SocketAddrV4::new(Ipv4Addr::new(192, 168, 1, 20), 27036);
        assert_eq!(lan, super::ConnectionId::client_visible_source(lan));
    }
}
