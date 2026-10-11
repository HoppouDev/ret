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
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::sync::mpsc;
use tracing::{debug, error};

use super::binary;
use super::datagram::TryRead;
use super::ipv4_packet_buffer::Ipv4PacketBuffer;
use super::router::Router;

const TAG: &str = "Client";

/// Number of packets queued for the client
///
/// When the queue is full, connections keep their packets in order until it has room (see
/// `Outbox`), and stop reading from the network while too many of them are waiting.
const QUEUE_CAPACITY: usize = 64;

/// Maximum amount of queued packets written to the client at once
const MAX_WRITE_LENGTH: usize = 256 * 1024;

/// Queue of IPv4 packets to send to the client
pub type ClientSender = mpsc::Sender<Vec<u8>>;

/// Relay the packets of a client (the device), until it disconnects
pub async fn run(id: u32, stream: TcpStream) {
    let (reader, writer) = stream.into_split();
    let (sender, receiver) = mpsc::channel(QUEUE_CAPACITY);
    // When one direction stops, the other is dropped: closing the queue and the router makes all
    // the connections of this client terminate.
    tokio::select! {
        result = write_loop(id, writer, receiver) => {
            if let Err(err) = result {
                error!(target: TAG, "Cannot write: [{:?}] {}", err.kind(), err);
            }
        }
        result = read_loop(reader, Router::new(sender)) => match result {
            Ok(()) => debug!(target: TAG, "EOF reached"),
            Err(err) => error!(target: TAG, "Cannot read: [{:?}] {}", err.kind(), err),
        },
    }
}

async fn write_loop(
    id: u32,
    mut writer: OwnedWriteHalf,
    mut receiver: mpsc::Receiver<Vec<u8>>,
) -> io::Result<()> {
    // the client expects its id before any packet
    writer.write_all(&binary::to_byte_array(id)).await?;
    debug!(target: TAG, "Client id #{} sent to client", id);

    let mut buf = Vec::with_capacity(MAX_WRITE_LENGTH);
    while let Some(packet) = receiver.recv().await {
        buf.extend_from_slice(&packet);
        while buf.len() < MAX_WRITE_LENGTH {
            match receiver.try_recv() {
                Ok(packet) => buf.extend_from_slice(&packet),
                Err(_) => break,
            }
        }
        writer.write_all(&buf).await?;
        buf.clear();
    }
    Ok(())
}

async fn read_loop(reader: OwnedReadHalf, mut router: Router) -> io::Result<()> {
    let mut buffer = Ipv4PacketBuffer::new();
    loop {
        reader.readable().await?;
        match buffer.read_from(&mut TryRead(&reader)) {
            Ok(true) => {
                while let Some(packet) = buffer.as_ipv4_packet() {
                    router.send_to_network(&packet);
                    buffer.next();
                }
                // read one chunk at a time, so that the connections can process their packets
                // meanwhile (readiness does not consume the task budget)
                tokio::task::yield_now().await;
            }
            // EOF
            Ok(false) => return Ok(()),
            Err(err) if err.kind() == io::ErrorKind::WouldBlock => (),
            Err(err) => return Err(err),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::relay::test_support::{DEVICE_IP, HOST_ALIAS_IP, UdpDatagram, within};
    use byteorder::{BigEndian, ByteOrder};
    use std::net::{Ipv4Addr, SocketAddrV4};
    use tokio::io::AsyncReadExt;
    use tokio::net::{TcpListener, UdpSocket};

    async fn read_packet(device: &mut TcpStream) -> Vec<u8> {
        let mut packet = vec![0u8; 4];
        within(device.read_exact(&mut packet)).await.unwrap();
        let length = usize::from(BigEndian::read_u16(&packet[2..4]));
        packet.resize(length, 0);
        within(device.read_exact(&mut packet[4..])).await.unwrap();
        packet
    }

    #[tokio::test]
    async fn relay_packets_of_the_device() {
        let tunnel = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
        let mut device = TcpStream::connect(tunnel.local_addr().unwrap())
            .await
            .unwrap();
        let (stream, _) = within(tunnel.accept()).await.unwrap();
        let client = tokio::spawn(run(0x01020304, stream));

        let mut id = [0u8; 4];
        within(device.read_exact(&mut id)).await.unwrap();
        assert_eq!([1, 2, 3, 4], id);

        let server = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
        let source = SocketAddrV4::new(DEVICE_IP, 50000);
        let destination = SocketAddrV4::new(HOST_ALIAS_IP, server.local_addr().unwrap().port());
        let request = UdpDatagram {
            source,
            destination,
            payload: b"ping".to_vec(),
        };
        // the device writes a stream of packets, not split on packet boundaries
        let raw = request.to_packet();
        device.write_all(&raw[..10]).await.unwrap();
        // let the relay read the incomplete packet before the rest arrives
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        device.write_all(&raw[10..]).await.unwrap();

        let mut buf = [0u8; 64];
        let (len, peer) = within(server.recv_from(&mut buf)).await.unwrap();
        assert_eq!(b"ping", &buf[..len]);
        server.send_to(b"pong", peer).await.unwrap();

        let response = UdpDatagram::parse(&read_packet(&mut device).await);
        assert_eq!(destination, response.source);
        assert_eq!(source, response.destination);
        assert_eq!(b"pong", &response.payload[..]);

        drop(device);
        within(client).await.unwrap();
    }
}
