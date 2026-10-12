//! Connections to the network on behalf of the clients, one task per connection

#[macro_use]
mod macros;
mod id;
mod outbox;
mod tcp;
mod udp;

pub use id::ConnectionId;
pub use tcp::TcpConnection;
pub use udp::UdpConnection;
