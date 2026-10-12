//! Blocking work of ret (relay server, adb), run outside of the UI thread

use anyhow::Context;
use iced::Subscription;
use iced::futures::{SinkExt, Stream};
use ret_adb::Adb;
use ret_adb::client::{self, StartOptions};
use ret_adb::track::{self, Device};
use std::fmt;
use std::thread;
use tokio::sync::{mpsc, oneshot};

/// Stops the relay server when dropped or when `stop()` is called
pub struct RelayHandle {
    stop: Option<oneshot::Sender<()>>,
}

impl RelayHandle {
    pub fn stop(&mut self) {
        if let Some(stop) = self.stop.take() {
            // the server already stopped by itself otherwise
            let _ = stop.send(());
        }
    }
}

/// Run the relay server on its own thread, return how to stop it and its result
///
/// The result comes once the server stops: after `RelayHandle::stop()` or on error (such as the
/// port being in use).
pub fn start_relay(port: u16) -> (RelayHandle, impl Future<Output = Result<(), String>>) {
    let (stop, stopped) = oneshot::channel::<()>();
    let (done, result) = oneshot::channel();
    thread::Builder::new()
        .name("relay".to_owned())
        .spawn(move || {
            let shutdown = async {
                // a dropped handle stops the server too
                let _ = stopped.await;
            };
            let result = ret_relay::relay_until(port, shutdown)
                .with_context(|| format!("cannot run the relay server on port {port}"));
            let _ = done.send(result);
        })
        .expect("cannot spawn the relay thread");
    let result = async move {
        match result.await {
            Ok(result) => result.map_err(|err| format!("{err:#}")),
            Err(_) => Err("the relay server crashed".to_owned()),
        }
    };
    (RelayHandle { stop: Some(stop) }, result)
}

/// The devices known to adb, each time the list changes
pub fn devices() -> Subscription<Vec<Device>> {
    Subscription::run(device_lists)
}

fn device_lists() -> impl Stream<Item = Vec<Device>> {
    iced::stream::channel(16, async |mut output| {
        let (lists, mut receiver) = mpsc::unbounded_channel();
        // the adb tracking is blocking and never ends, it lives as long as the window
        thread::Builder::new()
            .name("adb-devices".to_owned())
            .spawn(move || {
                let adb = Adb::from_env(None);
                track::watch_device_lists(&adb, |devices| {
                    let _ = lists.send(devices);
                })
            })
            .expect("cannot spawn the adb thread");
        while let Some(devices) = receiver.recv().await {
            if output.send(devices).await.is_err() {
                break;
            }
        }
    })
}

/// What can be done with the client of a device
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Start,
    Stop,
    Reinstall,
    Tunnel,
}

impl fmt::Display for Action {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(match self {
            Action::Start => "start",
            Action::Stop => "stop",
            Action::Reinstall => "reinstall",
            Action::Tunnel => "reset the tunnel of",
        })
    }
}

/// Apply `action` to the client of the device `serial`
pub async fn run(serial: String, action: Action, options: StartOptions) -> Result<(), String> {
    let context = format!("cannot {action} the client on {serial}");
    let work = move || -> anyhow::Result<()> {
        let adb = Adb::from_env(Some(serial));
        match action {
            Action::Start => client::start(&adb, &options)?,
            Action::Stop => client::stop(&adb)?,
            Action::Reinstall => {
                client::uninstall(&adb)?;
                client::install(&adb)?;
            }
            Action::Tunnel => client::tunnel(&adb, options.port)?,
        }
        Ok(())
    };
    tokio::task::spawn_blocking(work)
        .await
        .context("the adb task crashed")
        .and_then(|result| result)
        .with_context(|| context)
        .map_err(|err| format!("{err:#}"))
}
