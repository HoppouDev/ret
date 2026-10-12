//! State of the window and how messages change it

use iced::theme::Mode;
use iced::{Subscription, Task, Theme, system, time};
use ret_adb::client::StartOptions;
use ret_adb::track::Device;
use std::collections::HashSet;
use std::time::Duration;

use crate::backend::{self, Action, RelayHandle};
use crate::logs::{self, LogFilter, Logs};
use crate::style;

/// Port the relay server listens on by default, the same as the `ret` command
pub const DEFAULT_PORT: u16 = 31416;

/// Period of the animation of the work in progress
const TICK: Duration = Duration::from_millis(400);

#[derive(Debug, Clone)]
pub enum Message {
    PortChanged(String),
    DnsServersChanged(String),
    RoutesChanged(String),
    AutostartToggled(bool),
    ToggleRelay,
    RelayStopped(Result<(), String>),
    Devices(Vec<Device>),
    Run(String, Action),
    Done(String, Action, Result<(), String>),
    Log(logs::Line),
    ClearLogs,
    ToggleLogs,
    LogFilterChanged(LogFilter),
    ThemeChanged(Mode),
    Tick,
}

pub enum Relay {
    Stopped { error: Option<String> },
    Running { port: u16, handle: RelayHandle },
    Stopping,
}

/// A device known to adb, with the state of the work done on its client
pub struct DeviceRow {
    pub device: Device,
    /// The action in progress
    pub busy: Option<Action>,
    /// The outcome of the last action
    pub outcome: Option<Result<String, String>>,
}

pub struct Ret {
    pub port: String,
    pub dns_servers: String,
    pub routes: String,
    pub autostart: bool,
    pub relay: Relay,
    pub devices: Vec<DeviceRow>,
    pub logs: Logs,
    pub mode: Mode,
    /// Advances while some work is in progress, to animate it
    pub tick: usize,
}

impl Ret {
    pub fn new() -> Self {
        Self {
            port: DEFAULT_PORT.to_string(),
            dns_servers: String::new(),
            routes: String::new(),
            autostart: false,
            relay: Relay::Stopped { error: None },
            devices: Vec::new(),
            logs: Logs::default(),
            mode: Mode::None,
            tick: 0,
        }
    }

    /// The initial state, and the request of the system theme
    pub fn boot() -> (Self, Task<Message>) {
        (Self::new(), system::theme().map(Message::ThemeChanged))
    }

    pub fn theme(&self) -> Theme {
        style::theme(self.mode)
    }

    /// Whether some work is in progress, shown animated
    pub fn is_busy(&self) -> bool {
        matches!(self.relay, Relay::Stopping) || self.devices.iter().any(|row| row.busy.is_some())
    }

    /// The port typed in, if valid
    pub fn port(&self) -> Option<u16> {
        self.port.trim().parse().ok().filter(|&port| port != 0)
    }

    fn start_options(&self) -> StartOptions {
        let non_empty = |value: &str| Some(value.trim().to_owned()).filter(|v| !v.is_empty());
        StartOptions {
            dns_servers: non_empty(&self.dns_servers),
            routes: non_empty(&self.routes),
            port: self.port().unwrap_or(DEFAULT_PORT),
        }
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::PortChanged(port) => self.port = port,
            Message::DnsServersChanged(dns_servers) => self.dns_servers = dns_servers,
            Message::RoutesChanged(routes) => self.routes = routes,
            Message::AutostartToggled(autostart) => {
                self.autostart = autostart;
                if autostart {
                    let available = self.available_serials();
                    return self.run_all(available, Action::Start);
                }
            }
            Message::ToggleRelay => return self.toggle_relay(),
            Message::RelayStopped(result) => {
                self.relay = Relay::Stopped {
                    error: result.err(),
                };
            }
            Message::Devices(devices) => return self.update_devices(devices),
            Message::Run(serial, action) => return self.run(serial, action),
            Message::Done(serial, action, result) => {
                if let Some(row) = self.row_mut(&serial) {
                    row.busy = None;
                    row.outcome = Some(result.map(|()| done_message(action)));
                }
            }
            Message::Log(line) => self.logs.push(line),
            Message::ClearLogs => self.logs.clear(),
            Message::ToggleLogs => self.logs.visible = !self.logs.visible,
            Message::LogFilterChanged(filter) => self.logs.filter = filter,
            Message::ThemeChanged(mode) => self.mode = mode,
            Message::Tick => self.tick = self.tick.wrapping_add(1),
        }
        Task::none()
    }

    pub fn subscription(&self) -> Subscription<Message> {
        let tick = if self.is_busy() {
            time::every(TICK).map(|_| Message::Tick)
        } else {
            Subscription::none()
        };
        Subscription::batch([
            backend::devices().map(Message::Devices),
            logs::subscription().map(Message::Log),
            system::theme_changes().map(Message::ThemeChanged),
            tick,
        ])
    }

    fn toggle_relay(&mut self) -> Task<Message> {
        match &mut self.relay {
            Relay::Running { handle, .. } => {
                handle.stop();
                self.relay = Relay::Stopping;
                Task::none()
            }
            Relay::Stopped { .. } => {
                let Some(port) = self.port() else {
                    return Task::none();
                };
                let (handle, stopped) = backend::start_relay(port);
                self.relay = Relay::Running { port, handle };
                Task::perform(stopped, Message::RelayStopped)
            }
            Relay::Stopping => Task::none(),
        }
    }

    fn update_devices(&mut self, devices: Vec<Device>) -> Task<Message> {
        let before = self.available_serials();
        let rows = std::mem::take(&mut self.devices);
        self.devices = devices
            .into_iter()
            .map(|device| {
                // keep the work in progress and the outcome of the devices still there
                match rows
                    .iter()
                    .position(|row| row.device.serial == device.serial)
                {
                    Some(index) => DeviceRow {
                        device,
                        busy: rows[index].busy,
                        outcome: rows[index].outcome.clone(),
                    },
                    None => DeviceRow {
                        device,
                        busy: None,
                        outcome: None,
                    },
                }
            })
            .collect();
        if !self.autostart {
            return Task::none();
        }
        let before: HashSet<String> = before.into_iter().collect();
        let new = self
            .available_serials()
            .into_iter()
            .filter(|serial| !before.contains(serial))
            .collect();
        self.run_all(new, Action::Start)
    }

    fn available_serials(&self) -> Vec<String> {
        self.devices
            .iter()
            .filter(|row| row.device.is_available())
            .map(|row| row.device.serial.clone())
            .collect()
    }

    fn run_all(&mut self, serials: Vec<String>, action: Action) -> Task<Message> {
        Task::batch(serials.into_iter().map(|serial| self.run(serial, action)))
    }

    fn run(&mut self, serial: String, action: Action) -> Task<Message> {
        let options = self.start_options();
        let Some(row) = self.row_mut(&serial) else {
            return Task::none();
        };
        if row.busy.is_some() {
            return Task::none();
        }
        row.busy = Some(action);
        row.outcome = None;
        Task::perform(
            backend::run(serial.clone(), action, options),
            move |result| Message::Done(serial.clone(), action, result),
        )
    }

    fn row_mut(&mut self, serial: &str) -> Option<&mut DeviceRow> {
        self.devices
            .iter_mut()
            .find(|row| row.device.serial == serial)
    }
}

fn done_message(action: Action) -> String {
    match action {
        Action::Start => "Client started",
        Action::Stop => "Client stopped",
        Action::Reinstall => "Client reinstalled",
        Action::Tunnel => "Tunnel reset",
    }
    .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn device(serial: &str, state: &str) -> Device {
        Device {
            serial: serial.to_owned(),
            state: state.to_owned(),
        }
    }

    fn busy(ret: &Ret, serial: &str) -> Option<Action> {
        ret.devices
            .iter()
            .find(|row| row.device.serial == serial)
            .and_then(|row| row.busy)
    }

    #[test]
    fn autostart_starts_each_device_once_available() {
        let mut ret = Ret::new();
        let _ = ret.update(Message::AutostartToggled(true));
        let _ = ret.update(Message::Devices(vec![
            device("one", "device"),
            device("two", "unauthorized"),
        ]));
        assert_eq!(Some(Action::Start), busy(&ret, "one"));
        assert_eq!(None, busy(&ret, "two"));

        let _ = ret.update(Message::Done("one".to_owned(), Action::Start, Ok(())));
        // authorized meanwhile: only the new available device starts
        let _ = ret.update(Message::Devices(vec![
            device("one", "device"),
            device("two", "device"),
        ]));
        assert_eq!(None, busy(&ret, "one"));
        assert_eq!(Some(Action::Start), busy(&ret, "two"));
    }

    #[test]
    fn devices_are_not_started_without_autostart() {
        let mut ret = Ret::new();
        let _ = ret.update(Message::Devices(vec![device("one", "device")]));
        assert_eq!(None, busy(&ret, "one"));

        // enabling it starts the devices already there
        let _ = ret.update(Message::AutostartToggled(true));
        assert_eq!(Some(Action::Start), busy(&ret, "one"));
    }

    #[test]
    fn keep_work_in_progress_across_list_updates() {
        let mut ret = Ret::new();
        let _ = ret.update(Message::Devices(vec![device("one", "device")]));
        let _ = ret.update(Message::Run("one".to_owned(), Action::Reinstall));
        // a second action waits for the first one
        let _ = ret.update(Message::Run("one".to_owned(), Action::Stop));
        let _ = ret.update(Message::Devices(vec![
            device("one", "device"),
            device("two", "device"),
        ]));
        assert_eq!(Some(Action::Reinstall), busy(&ret, "one"));

        let _ = ret.update(Message::Done(
            "one".to_owned(),
            Action::Reinstall,
            Err("cannot reinstall".to_owned()),
        ));
        let row = &ret.devices[0];
        assert_eq!(None, row.busy);
        assert_eq!(Some(Err("cannot reinstall".to_owned())), row.outcome);
    }
}
