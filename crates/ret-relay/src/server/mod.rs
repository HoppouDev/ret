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
use tokio::runtime;
use tracing::info;

mod client;
mod router;
mod tunnel;

pub use client::ClientSender;

const TAG: &str = "Relay";

pub struct Relay {
    port: u16,
}

impl Relay {
    pub fn new(port: u16) -> Self {
        Self { port }
    }

    /// Relay until `shutdown` completes, then close every connection
    pub fn run(&self, shutdown: impl Future<Output = ()>) -> io::Result<()> {
        // all the connections are relayed on the current thread
        let runtime = runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        // returning drops the runtime, which cancels the tasks of the clients and their connections
        runtime.block_on(async {
            let listener = tunnel::bind(self.port).await?;
            info!(target: TAG, "Relay server started");
            tokio::select! {
                () = tunnel::serve(listener) => (),
                () = shutdown => info!(target: TAG, "Relay server stopped"),
            }
            Ok(())
        })
    }
}
