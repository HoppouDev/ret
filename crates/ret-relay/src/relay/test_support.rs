//! Builders and parsers for the IPv4 packets exchanged with the client, shared by the tests

use byteorder::{BigEndian, ByteOrder, WriteBytesExt};
use std::net::{Ipv4Addr, SocketAddrV4};
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time;

use super::connection::ConnectionId;
use super::ipv4_packet::Ipv4Packet;

/// Address of the device in the tunnel
pub const DEVICE_IP: Ipv4Addr = Ipv4Addr::new(10, 0, 0, 2);
/// Address of the host as seen from the Android emulator, relayed to localhost
pub const HOST_ALIAS_IP: Ipv4Addr = Ipv4Addr::new(10, 0, 2, 2);

const PROTOCOL_TCP: u8 = 6;
const PROTOCOL_UDP: u8 = 17;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TcpSegment {
    pub source: SocketAddrV4,
    pub destination: SocketAddrV4,
    pub seq: u32,
    pub ack: u32,
    pub flags: u16,
    pub window: u16,
    pub payload: Vec<u8>,
}

impl TcpSegment {
    pub fn to_packet(&self) -> Vec<u8> {
        let mut raw = ipv4_header(
            PROTOCOL_TCP,
            self.source,
            self.destination,
            20 + self.payload.len(),
        );
        write_ports(&mut raw, self.source, self.destination);
        raw.write_u32::<BigEndian>(self.seq).unwrap();
        raw.write_u32::<BigEndian>(self.ack).unwrap();
        raw.write_u16::<BigEndian>(5 << 12 | self.flags).unwrap();
        raw.write_u16::<BigEndian>(self.window).unwrap();
        raw.write_u16::<BigEndian>(0).unwrap(); // checksum
        raw.write_u16::<BigEndian>(0).unwrap(); // urgent pointer
        raw.extend_from_slice(&self.payload);
        Ipv4Packet::parse(&mut raw).compute_checksums();
        raw
    }

    /// Parse a packet built by the relay, asserting its checksums are valid
    pub fn parse(raw: &[u8]) -> Self {
        assert_eq!(PROTOCOL_TCP, raw[9], "Not a TCP packet");
        assert_valid_checksums(raw);
        let (source, destination) = addresses(raw);
        let tcp = &raw[20..];
        let header_length = usize::from(tcp[12] >> 4) * 4;
        Self {
            source,
            destination,
            seq: BigEndian::read_u32(&tcp[4..8]),
            ack: BigEndian::read_u32(&tcp[8..12]),
            flags: BigEndian::read_u16(&tcp[12..14]) & 0x1FF,
            window: BigEndian::read_u16(&tcp[14..16]),
            payload: tcp[header_length..].to_vec(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UdpDatagram {
    pub source: SocketAddrV4,
    pub destination: SocketAddrV4,
    pub payload: Vec<u8>,
}

impl UdpDatagram {
    pub fn to_packet(&self) -> Vec<u8> {
        let udp_length = 8 + self.payload.len();
        let mut raw = ipv4_header(PROTOCOL_UDP, self.source, self.destination, udp_length);
        write_ports(&mut raw, self.source, self.destination);
        raw.write_u16::<BigEndian>(udp_length as u16).unwrap();
        raw.write_u16::<BigEndian>(0).unwrap(); // checksum (disabled)
        raw.extend_from_slice(&self.payload);
        Ipv4Packet::parse(&mut raw).compute_checksums();
        raw
    }

    /// Parse a packet built by the relay, asserting its headers are consistent
    pub fn parse(raw: &[u8]) -> Self {
        assert_eq!(PROTOCOL_UDP, raw[9], "Not a UDP packet");
        assert_valid_checksums(raw);
        let (source, destination) = addresses(raw);
        let udp = &raw[20..];
        assert_eq!(
            udp.len(),
            usize::from(BigEndian::read_u16(&udp[4..6])),
            "UDP length does not match the IPv4 total length"
        );
        Self {
            source,
            destination,
            payload: udp[8..].to_vec(),
        }
    }
}

/// Build a packet carrying an arbitrary transport protocol (not TCP nor UDP)
pub fn other_protocol_packet(
    protocol: u8,
    source: SocketAddrV4,
    destination: SocketAddrV4,
) -> Vec<u8> {
    let mut raw = ipv4_header(protocol, source, destination, 8);
    raw.extend_from_slice(&[0; 8]);
    raw
}

pub fn connection_id(raw: &mut [u8]) -> ConnectionId {
    let ipv4_packet = Ipv4Packet::parse(raw);
    let (ipv4_header_data, transport_header_data) = ipv4_packet.headers_data();
    ConnectionId::from_headers(ipv4_header_data, transport_header_data.unwrap())
}

/// Fail the test instead of hanging when the relay does not react
pub async fn within<F: Future>(future: F) -> F::Output {
    time::timeout(Duration::from_secs(5), future)
        .await
        .expect("Timed out")
}

/// Receive the next packet the relay sends to the client
pub async fn next_packet(receiver: &mut mpsc::Receiver<Vec<u8>>) -> Vec<u8> {
    within(receiver.recv()).await.expect("Client queue closed")
}

fn ipv4_header(
    protocol: u8,
    source: SocketAddrV4,
    destination: SocketAddrV4,
    transport_length: usize,
) -> Vec<u8> {
    let total_length = 20 + transport_length;
    let mut raw = Vec::with_capacity(total_length);
    raw.write_u8(4 << 4 | 5).unwrap(); // version and IHL
    raw.write_u8(0).unwrap(); // ToS
    raw.write_u16::<BigEndian>(total_length as u16).unwrap();
    raw.write_u32::<BigEndian>(0).unwrap(); // id, flags, fragment offset
    raw.write_u8(64).unwrap(); // TTL
    raw.write_u8(protocol).unwrap();
    raw.write_u16::<BigEndian>(0).unwrap(); // checksum
    raw.extend_from_slice(&source.ip().octets());
    raw.extend_from_slice(&destination.ip().octets());
    raw
}

fn write_ports(raw: &mut Vec<u8>, source: SocketAddrV4, destination: SocketAddrV4) {
    raw.write_u16::<BigEndian>(source.port()).unwrap();
    raw.write_u16::<BigEndian>(destination.port()).unwrap();
}

fn addresses(raw: &[u8]) -> (SocketAddrV4, SocketAddrV4) {
    let ip = |range: std::ops::Range<usize>| Ipv4Addr::from(BigEndian::read_u32(&raw[range]));
    (
        SocketAddrV4::new(ip(12..16), BigEndian::read_u16(&raw[20..22])),
        SocketAddrV4::new(ip(16..20), BigEndian::read_u16(&raw[22..24])),
    )
}

fn assert_valid_checksums(raw: &[u8]) {
    assert_eq!(
        raw.len(),
        usize::from(BigEndian::read_u16(&raw[2..4])),
        "IPv4 total length does not match the packet length"
    );
    let mut recomputed = raw.to_vec();
    Ipv4Packet::parse(&mut recomputed).compute_checksums();
    assert_eq!(raw, &recomputed[..], "Invalid checksums");
}
