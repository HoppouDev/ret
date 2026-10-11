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
use std::net::SocketAddrV4;

use super::ipv4_header::{Ipv4HeaderData, Protocol};
use super::net;
use super::transport_header::TransportHeaderData;

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
            "{} -> {}",
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
}

impl fmt::Display for ConnectionId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.id_string)
    }
}

// macros to log connection id along with the message

macro_rules! cx_format {
    ($id:tt, $str:tt, $($arg:tt)+) => {
        format!(concat!("{} ", $str), $id, $($arg)+)
    };
    ($id:tt, $str:tt) => {
        format!(concat!("{} ", $str), $id)
    };
}

macro_rules! cx_trace {
    (target: $target:expr, $id:expr, $($arg:tt)*) => {
        tracing::trace!(target: $target, "{}", cx_format!($id, $($arg)+))
    }
}

macro_rules! cx_debug {
    (target: $target:expr, $id:expr, $($arg:tt)*) => {
        tracing::debug!(target: $target, "{}", cx_format!($id, $($arg)+))
    }
}

macro_rules! cx_info {
    (target: $target:expr, $id:expr, $($arg:tt)*) => {
        tracing::info!(target: $target, "{}", cx_format!($id, $($arg)+))
    }
}

macro_rules! cx_warn {
    (target: $target:expr, $id:expr, $($arg:tt)*) => {
        tracing::warn!(target: $target, "{}", cx_format!($id, $($arg)+))
    }
}

macro_rules! cx_error {
    (target: $target:expr, $id:expr, $($arg:tt)*) => {
        tracing::error!(target: $target, "{}", cx_format!($id, $($arg)+))
    }
}

#[cfg(test)]
mod tests {
    use crate::relay::test_support::{DEVICE_IP, HOST_ALIAS_IP, UdpDatagram, connection_id};
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
}
