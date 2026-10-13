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

//! Relay server of ret: forwards the IP packets of the Android clients to the network

mod binary;
mod buffer;
#[macro_use]
mod connection;
mod datagram;
mod net;
mod packet;
mod server;
#[cfg(test)]
mod test_support;

use crate::server::Relay;

/// Run the relay server on `port` of localhost, until an error occurs
///
/// The clients reach it through `adb reverse`. All the connections are relayed on the
/// current thread.
pub fn relay(port: u16) -> anyhow::Result<()> {
    relay_until(port, std::future::pending())
}

/// Run the relay server like [`relay`], until `shutdown` completes
///
/// Then every client and every connection is closed.
pub fn relay_until(port: u16, shutdown: impl Future<Output = ()>) -> anyhow::Result<()> {
    Relay::new(port).run(shutdown)
}
