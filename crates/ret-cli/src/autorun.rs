//! `ret autorun`: start the client on every device, and run the relay server only while a client
//! needs it

use ret_adb::client::{self, StartOptions};
use ret_adb::{Adb, track};
use std::collections::HashMap;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};
use tokio::sync::oneshot;
use tracing::{error, info, warn};

/// Delay between two checks of the clients, which can also be started or stopped from the devices
const CHECK_INTERVAL: Duration = Duration::from_secs(5);

/// Time a started client has to bring its VPN up, the user may have to accept it on the device
const LAUNCH_GRACE: Duration = Duration::from_secs(30);

enum Event {
    /// The serials of the devices adb can use, each time the list changes
    Devices(Vec<String>),
    /// Whether the client of a device was started
    Started(String, bool),
    /// A relay server stopped, with its error unless it was asked to
    RelayExited { id: u64, error: Option<String> },
}

/// Start the client on every device adb detects, and run the relay server while one runs, forever
pub fn autorun(options: StartOptions) -> Result<(), String> {
    let (events, receiver) = mpsc::channel();
    watch_devices(events.clone());

    let adb = Adb::from_env(None);
    let mut clients = Clients::default();
    let mut relay: Option<RelayServer> = None;
    let mut next_relay_id = 0;
    let mut next_check = Instant::now() + CHECK_INTERVAL;
    loop {
        match receiver.recv_timeout(next_check.saturating_duration_since(Instant::now())) {
            Ok(Event::Devices(available)) => {
                for serial in clients.update(available) {
                    spawn_start(&adb, serial, options.clone(), events.clone());
                }
            }
            Ok(Event::Started(serial, started)) => {
                let client = if started {
                    Client::Launched(Instant::now())
                } else {
                    Client::Stopped
                };
                clients.set(&serial, client);
            }
            Ok(Event::RelayExited { id, error }) => {
                // a relay stopped on purpose reports too, maybe once another one runs
                if relay.as_ref().is_some_and(|server| server.id == id) {
                    return Err(error.unwrap_or_else(|| "the relay server stopped".to_owned()));
                }
            }
            Err(RecvTimeoutError::Timeout) => {
                for serial in clients.settled() {
                    match client::is_running(&adb.with_serial(&serial)) {
                        Ok(running) => clients.checked(&serial, running, Instant::now()),
                        Err(err) => warn!("Cannot check the client of {serial}: {err}"),
                    }
                }
                next_check = Instant::now() + CHECK_INTERVAL;
            }
            // this loop holds a sender
            Err(RecvTimeoutError::Disconnected) => unreachable!(),
        }

        match (clients.relay_wanted(), relay.take()) {
            (true, None) => {
                relay = Some(RelayServer::start(
                    next_relay_id,
                    options.port,
                    events.clone(),
                ));
                next_relay_id += 1;
            }
            (false, Some(server)) => server.stop(),
            (_, server) => relay = server,
        }
    }
}

/// Send the available devices to `events` whenever adb reports a change
fn watch_devices(events: mpsc::Sender<Event>) {
    thread::Builder::new()
        .name("adb-devices".to_owned())
        .spawn(move || {
            let adb = Adb::from_env(None);
            track::watch_device_lists(&adb, |devices| {
                let available = devices
                    .into_iter()
                    .filter(|device| device.is_available())
                    .map(|device| device.serial)
                    .collect();
                let _ = events.send(Event::Devices(available));
            })
        })
        .expect("cannot spawn the adb thread");
}

/// Start the client in the background, then report whether it started to `events`
fn spawn_start(adb: &Adb, serial: String, options: StartOptions, events: mpsc::Sender<Event>) {
    let adb = adb.with_serial(&serial);
    thread::spawn(move || {
        let started = match client::start(&adb, &options) {
            Ok(()) => true,
            Err(err) => {
                error!("Cannot start the client on {serial}: {err}");
                false
            }
        };
        let _ = events.send(Event::Started(serial, started));
    });
}

/// The relay server, running on its own thread
struct RelayServer {
    id: u64,
    stop: oneshot::Sender<()>,
    thread: JoinHandle<()>,
}

impl RelayServer {
    fn start(id: u64, port: u16, events: mpsc::Sender<Event>) -> Self {
        info!("A client runs, starting the relay server on port {port}");
        let (stop, stopped) = oneshot::channel::<()>();
        let thread = thread::Builder::new()
            .name("relay".to_owned())
            .spawn(move || {
                // reports the end of the thread, a panic included
                let mut exit = ExitReport {
                    id,
                    events,
                    error: Some("the relay server crashed".to_owned()),
                };
                let shutdown = async {
                    let _ = stopped.await;
                };
                exit.error = ret_relay::relay_until(port, shutdown)
                    .err()
                    .map(|err| format!("cannot run the relay server: {err:#}"));
            })
            .expect("cannot spawn the relay thread");
        Self { id, stop, thread }
    }

    /// Stop the server and wait for it to release its port, so that it can be started again
    fn stop(self) {
        info!("No client runs anymore, stopping the relay server");
        let _ = self.stop.send(());
        let _ = self.thread.join();
    }
}

/// Sends [`Event::RelayExited`] when dropped, as the relay thread ends
struct ExitReport {
    id: u64,
    events: mpsc::Sender<Event>,
    error: Option<String>,
}

impl Drop for ExitReport {
    fn drop(&mut self) {
        let _ = self.events.send(Event::RelayExited {
            id: self.id,
            error: self.error.take(),
        });
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Client {
    /// Being installed and started
    Starting,
    /// Started at this instant, its VPN not seen up yet
    Launched(Instant),
    Running,
    Stopped,
}

/// The client of each available device
#[derive(Default)]
struct Clients(HashMap<String, Client>);

impl Clients {
    /// Follow the `available` devices, return the new ones, whose client is then starting
    fn update(&mut self, available: Vec<String>) -> Vec<String> {
        self.0.retain(|serial, _| available.contains(serial));
        let mut new = Vec::new();
        for serial in available {
            if !self.0.contains_key(&serial) {
                self.0.insert(serial.clone(), Client::Starting);
                new.push(serial);
            }
        }
        new
    }

    fn set(&mut self, serial: &str, client: Client) {
        if let Some(current) = self.0.get_mut(serial) {
            *current = client;
        }
    }

    /// The devices whose client is not being started, so whose state can be checked
    fn settled(&self) -> Vec<String> {
        self.0
            .iter()
            .filter(|(_, client)| **client != Client::Starting)
            .map(|(serial, _)| serial.clone())
            .collect()
    }

    /// Record a check at `now` of whether a client runs
    ///
    /// A client being started is left alone, and one just launched is given [`LAUNCH_GRACE`] to
    /// bring its VPN up before it counts as stopped.
    fn checked(&mut self, serial: &str, running: bool, now: Instant) {
        let Some(client) = self.0.get_mut(serial) else {
            return;
        };
        *client = match (*client, running) {
            (Client::Starting, _) => Client::Starting,
            (_, true) => Client::Running,
            (Client::Launched(at), false) if now.duration_since(at) < LAUNCH_GRACE => *client,
            (_, false) => Client::Stopped,
        };
    }

    /// Whether the relay server is needed: some client runs, or is starting or launched
    fn relay_wanted(&self) -> bool {
        self.0.values().any(|client| *client != Client::Stopped)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn serials(serials: &[&str]) -> Vec<String> {
        serials.iter().map(|serial| (*serial).to_owned()).collect()
    }

    #[test]
    fn start_each_new_device_once() {
        let mut clients = Clients::default();
        assert_eq!(serials(&["one"]), clients.update(serials(&["one"])));
        assert_eq!(serials(&["two"]), clients.update(serials(&["one", "two"])));
        assert!(clients.update(serials(&["one", "two"])).is_empty());

        // a device plugged back is started again
        clients.update(serials(&["two"]));
        assert_eq!(serials(&["one"]), clients.update(serials(&["one", "two"])));
    }

    #[test]
    fn want_the_relay_while_a_client_runs_or_starts() {
        let now = Instant::now();
        let mut clients = Clients::default();
        assert!(!clients.relay_wanted());
        clients.update(serials(&["one", "two"]));
        assert!(clients.relay_wanted());

        clients.set("one", Client::Stopped);
        clients.set("two", Client::Running);
        assert!(clients.relay_wanted());
        // stopped from the device
        clients.checked("two", false, now);
        assert!(!clients.relay_wanted());
        // started from the device or another terminal
        clients.checked("one", true, now);
        assert!(clients.relay_wanted());
        // unplugged
        clients.update(serials(&["two"]));
        assert!(!clients.relay_wanted());
    }

    #[test]
    fn a_check_does_not_override_a_start_in_progress() {
        let mut clients = Clients::default();
        clients.update(serials(&["one"]));
        assert!(clients.settled().is_empty());
        clients.checked("one", false, Instant::now());
        assert!(clients.relay_wanted());
    }

    #[test]
    fn give_a_launched_client_time_to_bring_its_vpn_up() {
        let launched = Instant::now();
        let mut clients = Clients::default();
        clients.update(serials(&["one"]));
        clients.set("one", Client::Launched(launched));

        // the VPN is not up yet, or the user has not accepted it
        clients.checked("one", false, launched + Duration::from_secs(5));
        assert!(clients.relay_wanted());
        // never came up
        clients.checked("one", false, launched + LAUNCH_GRACE);
        assert!(!clients.relay_wanted());

        // once seen up, a later stop counts at once
        clients.set("one", Client::Launched(launched));
        clients.checked("one", true, launched + Duration::from_secs(5));
        clients.checked("one", false, launched + Duration::from_secs(10));
        assert!(!clients.relay_wanted());
    }
}
