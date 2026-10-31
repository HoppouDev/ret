//! State of the window and how messages change it

use iced::theme::Mode;
use iced::{Subscription, Task, Theme, system, time};
use ret_adb::client::StartOptions;
use ret_adb::download::Progress;
use ret_adb::info::DeviceInfo;
use ret_adb::track::Device;
use std::collections::HashSet;
use std::time::Duration;
use tracing::warn;

use crate::backend::{self, Action, RelayHandle};
use crate::config::{Config, ConfigFile, ThemeChoice};
use crate::logs::{self, LogFilter, Logs};
use crate::style;

/// Port the relay server listens on by default, the same as the `ret` command
pub const DEFAULT_PORT: u16 = 31416;

/// Period of the animation of the work in progress
const TICK: Duration = Duration::from_millis(400);

/// Why a client cannot start, nor the relay serve it
const INVALID_PORT: &str = "The port in the settings is invalid, enter one between 1 and 65535";

#[derive(Debug, Clone)]
pub enum Message {
    PortChanged(String),
    DnsServersChanged(String),
    RoutesChanged(String),
    AutostartToggled(bool),
    /// The relay server stopped, by itself on error or once no client needed it
    RelayStopped(Result<(), String>),
    Devices(Vec<Device>),
    Download(Progress),
    Run(String, Action),
    Done(String, Action, Result<(), String>),
    DeviceInfo(String, DeviceInfo),
    ClientRunning(String, bool),
    /// Open the menu of a device, or close it if it is the one open
    ToggleMenu(String),
    CloseMenu,
    Logs(Vec<logs::Line>),
    ClearLogs,
    ToggleLogs,
    LogFilterChanged(LogFilter),
    /// The system switched between light and dark
    ThemeChanged(Mode),
    ThemeChosen(ThemeChoice),
    ShowPage(Page),
    Tick,
}

/// What the window shows
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    /// The relay, the devices and the logs
    Home,
    Settings,
}

pub enum Relay {
    Stopped { error: Option<String> },
    Running { port: u16, handle: RelayHandle },
    Stopping,
}

/// A device known to adb, with the state of the work done on its client
pub struct DeviceRow {
    pub device: Device,
    /// Its name and kind, known once it was available
    pub info: Option<DeviceInfo>,
    /// Whether its client runs, `None` until known
    pub running: Option<bool>,
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
    pub theme_choice: ThemeChoice,
    pub page: Page,
    pub relay: Relay,
    pub devices: Vec<DeviceRow>,
    pub download: Progress,
    pub logs: Logs,
    /// Light or dark, as the system is
    pub mode: Mode,
    /// Advances while some work is in progress, to animate it
    pub tick: usize,
    /// The serial of the device whose menu is open
    pub menu: Option<String>,
    /// Where the settings are saved, `None` to keep them in memory only
    config_file: Option<ConfigFile>,
    /// The settings as last loaded or saved
    config: Config,
}

impl Ret {
    fn with_config(config: Config, config_file: Option<ConfigFile>) -> Self {
        Self {
            port: config.port.to_string(),
            dns_servers: config.dns_servers.join(","),
            routes: config.routes.join(","),
            autostart: config.autostart,
            theme_choice: config.theme,
            page: Page::Home,
            relay: Relay::Stopped { error: None },
            devices: Vec::new(),
            download: Progress::Idle,
            logs: Logs::default(),
            mode: Mode::None,
            tick: 0,
            menu: None,
            config_file,
            config,
        }
    }

    /// The initial state with the saved settings, and the request of the system theme
    pub fn boot() -> (Self, Task<Message>) {
        let (config_file, config) = match ConfigFile::default_path() {
            Some(path) => {
                let (file, config) = ConfigFile::load(path);
                (Some(file), config)
            }
            None => {
                warn!("No configuration directory, the settings will not be saved");
                (None, Config::default())
            }
        };
        let ret = Self::with_config(config, config_file);
        (ret, system::theme().map(Message::ThemeChanged))
    }

    /// Save the settings if they changed
    ///
    /// While the port typed in is invalid, the last valid one stays saved.
    fn save_config(&mut self) {
        let config = Config {
            port: self.port().unwrap_or(self.config.port),
            dns_servers: split_list(&self.dns_servers),
            routes: split_list(&self.routes),
            autostart: self.autostart,
            theme: self.theme_choice,
            ..Config::default()
        };
        if config != self.config {
            if let Some(file) = &self.config_file {
                let _ = file.save(&config);
            }
            self.config = config;
        }
    }

    pub fn theme(&self) -> Theme {
        style::theme(match self.theme_choice {
            ThemeChoice::System => self.mode,
            ThemeChoice::Light => Mode::Light,
            ThemeChoice::Dark => Mode::Dark,
        })
    }

    /// Whether some work is in progress, shown animated
    pub fn is_busy(&self) -> bool {
        matches!(self.relay, Relay::Stopping)
            || matches!(
                self.download,
                Progress::Downloading { .. } | Progress::Extracting
            )
            || self.devices.iter().any(|row| row.busy.is_some())
    }

    /// The port typed in, if valid
    pub fn port(&self) -> Option<u16> {
        self.port.trim().parse().ok().filter(|&port| port != 0)
    }

    fn start_options(&self, port: u16) -> StartOptions {
        let non_empty = |value: &str| Some(value.trim().to_owned()).filter(|v| !v.is_empty());
        StartOptions {
            dns_servers: non_empty(&self.dns_servers),
            routes: non_empty(&self.routes),
            port,
        }
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        let task = self.handle(message);
        Task::batch([task, self.sync_relay()])
    }

    fn handle(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::PortChanged(port) => {
                self.port = port;
                self.save_config();
                // another port deserves another try of a relay which failed
                if matches!(self.relay, Relay::Stopped { error: Some(_) }) && self.port().is_some()
                {
                    self.relay = Relay::Stopped { error: None };
                }
            }
            Message::DnsServersChanged(dns_servers) => {
                self.dns_servers = dns_servers;
                self.save_config();
            }
            Message::RoutesChanged(routes) => {
                self.routes = routes;
                self.save_config();
            }
            Message::AutostartToggled(autostart) => {
                self.autostart = autostart;
                self.save_config();
                if autostart {
                    let available = self.available_serials();
                    return self.run_all(available, Action::Start);
                }
            }
            Message::RelayStopped(result) => {
                self.relay = Relay::Stopped {
                    error: result.err(),
                };
            }
            Message::Devices(devices) => return self.update_devices(devices),
            Message::Download(progress) => self.download = progress,
            Message::Run(serial, action) => {
                self.menu = None;
                // starting a client again retries a relay which failed
                if action == Action::Start && matches!(self.relay, Relay::Stopped { .. }) {
                    self.relay = Relay::Stopped { error: None };
                }
                return self.run(serial, action);
            }
            Message::Done(serial, action, result) => {
                let Some(row) = self.row_mut(&serial) else {
                    return Task::none();
                };
                row.busy = None;
                if result.is_ok() {
                    row.running = running_after(action).or(row.running);
                }
                row.outcome = Some(result.map(|()| done_message(action)));
                // a check answered while the action ran was ignored, and the action did not tell
                if row.running.is_none() {
                    return query_running(serial);
                }
            }
            Message::DeviceInfo(serial, info) => {
                if let Some(row) = self.row_mut(&serial) {
                    row.info = Some(info);
                }
            }
            Message::ClientRunning(serial, running) => {
                if let Some(row) = self.row_mut(&serial) {
                    // an action done meanwhile knows better than this earlier check
                    if row.busy.is_none() && row.running.is_none() {
                        row.running = Some(running);
                    }
                }
            }
            Message::ToggleMenu(serial) => {
                self.menu = if self.menu.as_ref() == Some(&serial) {
                    None
                } else {
                    Some(serial)
                };
            }
            Message::CloseMenu => self.menu = None,
            Message::Logs(lines) => self.logs.extend(lines),
            Message::ClearLogs => self.logs.clear(),
            Message::ToggleLogs => self.logs.visible = !self.logs.visible,
            Message::LogFilterChanged(filter) => self.logs.filter = filter,
            Message::ThemeChanged(mode) => self.mode = mode,
            Message::ThemeChosen(choice) => {
                self.theme_choice = choice;
                self.save_config();
            }
            Message::ShowPage(page) => {
                self.menu = None;
                self.page = page;
            }
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
            backend::devices().map(|event| match event {
                backend::Event::Devices(devices) => Message::Devices(devices),
                backend::Event::Download(progress) => Message::Download(progress),
            }),
            logs::subscription().map(Message::Logs),
            system::theme_changes().map(Message::ThemeChanged),
            tick,
        ])
    }

    /// Whether the relay server is needed: some client runs or is starting
    fn relay_wanted(&self) -> bool {
        self.devices
            .iter()
            .any(|row| row.running == Some(true) || row.busy == Some(Action::Start))
    }

    /// Start the relay server while it is wanted, and stop it once it is not
    ///
    /// A relay which failed is not started again until a client is started, so that a port in use
    /// is not retried on every message.
    fn sync_relay(&mut self) -> Task<Message> {
        let wanted = self.relay_wanted();
        let port = self.port();
        match &mut self.relay {
            Relay::Stopped { error: None } if wanted => {
                let Some(port) = port else {
                    // a client found running needs the relay, which cannot start
                    self.relay = Relay::Stopped {
                        error: Some(INVALID_PORT.to_owned()),
                    };
                    return Task::none();
                };
                let (handle, stopped) = backend::start_relay(port);
                self.relay = Relay::Running { port, handle };
                Task::perform(stopped, Message::RelayStopped)
            }
            Relay::Running { handle, .. } if !wanted => {
                handle.stop();
                self.relay = Relay::Stopping;
                Task::none()
            }
            _ => Task::none(),
        }
    }

    fn update_devices(&mut self, devices: Vec<Device>) -> Task<Message> {
        let before = self.available_serials();
        let rows = std::mem::take(&mut self.devices);
        self.devices = devices
            .into_iter()
            .map(|device| {
                // keep what is known and done about the devices still there
                match rows.iter().find(|row| row.device.serial == device.serial) {
                    Some(row) => DeviceRow {
                        device,
                        info: row.info.clone(),
                        running: row.running,
                        busy: row.busy,
                        outcome: row.outcome.clone(),
                    },
                    None => DeviceRow {
                        device,
                        info: None,
                        running: None,
                        busy: None,
                        outcome: None,
                    },
                }
            })
            .collect();
        let before: HashSet<String> = before.into_iter().collect();
        let new: Vec<String> = self
            .available_serials()
            .into_iter()
            .filter(|serial| !before.contains(serial))
            .collect();
        if self
            .menu
            .as_ref()
            .is_some_and(|serial| self.row(serial).is_none())
        {
            self.menu = None;
        }
        let mut tasks = Vec::new();
        // an unauthorized device answers nothing, so ask once it becomes available
        for serial in &new {
            let Some(row) = self.row_mut(serial) else {
                continue;
            };
            // the client may have been started or stopped while the device was away
            row.running = None;
            if row.info.is_none() {
                let serial = serial.clone();
                tasks.push(
                    Task::future(backend::device_info(serial.clone())).and_then(move |info| {
                        Task::done(Message::DeviceInfo(serial.clone(), info))
                    }),
                );
            }
            tasks.push(query_running(serial.clone()));
        }
        if self.autostart {
            tasks.push(self.run_all(new, Action::Start));
        }
        Task::batch(tasks)
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
        let port = self.port();
        // stopping and reinstalling do not use the port
        let options = self.start_options(port.unwrap_or(DEFAULT_PORT));
        let Some(row) = self.row_mut(&serial) else {
            return Task::none();
        };
        if row.busy.is_some() {
            return Task::none();
        }
        // the client would connect to a port no relay listens on
        if port.is_none() && matches!(action, Action::Start | Action::Tunnel) {
            row.outcome = Some(Err(INVALID_PORT.to_owned()));
            return Task::none();
        }
        row.busy = Some(action);
        row.outcome = None;
        Task::perform(
            backend::run(serial.clone(), action, options),
            move |result| Message::Done(serial.clone(), action, result),
        )
    }

    fn row(&self, serial: &str) -> Option<&DeviceRow> {
        self.devices.iter().find(|row| row.device.serial == serial)
    }

    fn row_mut(&mut self, serial: &str) -> Option<&mut DeviceRow> {
        self.devices
            .iter_mut()
            .find(|row| row.device.serial == serial)
    }
}

/// The values of a comma-separated list, without blanks
fn split_list(list: &str) -> Vec<String> {
    list.split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .collect()
}

/// Ask whether the client of the device `serial` runs
fn query_running(serial: String) -> Task<Message> {
    Task::future(backend::client_running(serial.clone()))
        .and_then(move |running| Task::done(Message::ClientRunning(serial.clone(), running)))
}

/// Whether the client runs once `action` succeeded, `None` if it does not change
fn running_after(action: Action) -> Option<bool> {
    match action {
        Action::Start => Some(true),
        // uninstalling closes the VPN of the client
        Action::Stop | Action::Reinstall => Some(false),
        Action::Tunnel => None,
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

    /// A free port of this computer, for a relay that a test starts
    fn free_port() -> u16 {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.local_addr().unwrap().port()
    }

    /// A window which starts the devices only when asked, whatever the default settings
    ///
    /// Its relay listens on a free port when a test starts a client, and stops with the window.
    fn manual() -> Ret {
        let config = Config {
            autostart: false,
            port: free_port(),
            ..Config::default()
        };
        Ret::with_config(config, None)
    }

    #[test]
    fn want_the_relay_while_a_client_runs_or_starts() {
        let done = |serial: &str, action| Message::Done(serial.to_owned(), action, Ok(()));
        let mut ret = manual();
        let _ = ret.update(Message::Devices(vec![
            device("one", "device"),
            device("two", "device"),
        ]));
        assert!(!ret.relay_wanted());

        let _ = ret.update(Message::Run("one".to_owned(), Action::Start));
        assert!(ret.relay_wanted());
        assert!(matches!(ret.relay, Relay::Running { .. }));
        let _ = ret.update(done("one", Action::Start));
        let _ = ret.update(Message::ClientRunning("two".to_owned(), true));
        assert!(ret.relay_wanted());

        // needed until the last client stops
        let _ = ret.update(done("one", Action::Stop));
        assert!(ret.relay_wanted());
        let _ = ret.update(done("two", Action::Stop));
        assert!(!ret.relay_wanted());
        assert!(matches!(ret.relay, Relay::Stopping));
    }

    #[test]
    fn need_a_valid_port_to_start_a_client() {
        let mut ret = manual();
        let _ = ret.update(Message::Devices(vec![device("one", "device")]));
        let _ = ret.update(Message::PortChanged("40x".to_owned()));

        // the client is not started, and the row says why
        let _ = ret.update(Message::Run("one".to_owned(), Action::Start));
        assert_eq!(None, busy(&ret, "one"));
        assert_eq!(
            Some(Err(INVALID_PORT.to_owned())),
            ret.devices[0].outcome.clone()
        );
        // stopping does not need the port
        let _ = ret.update(Message::Run("one".to_owned(), Action::Stop));
        assert_eq!(Some(Action::Stop), busy(&ret, "one"));
    }

    #[test]
    fn report_a_client_found_running_without_a_valid_port() {
        let mut ret = manual();
        let _ = ret.update(Message::PortChanged(String::new()));
        let _ = ret.update(Message::Devices(vec![device("one", "device")]));
        let _ = ret.update(Message::ClientRunning("one".to_owned(), true));
        assert!(
            matches!(&ret.relay, Relay::Stopped { error: Some(error) } if error == INVALID_PORT)
        );

        // a valid port tries again
        let _ = ret.update(Message::PortChanged(free_port().to_string()));
        assert!(matches!(ret.relay, Relay::Running { .. }));
    }

    #[test]
    fn autostart_starts_each_device_once_available() {
        let mut ret = manual();
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
        let mut ret = manual();
        let _ = ret.update(Message::Devices(vec![device("one", "device")]));
        assert_eq!(None, busy(&ret, "one"));

        // enabling it starts the devices already there
        let _ = ret.update(Message::AutostartToggled(true));
        assert_eq!(Some(Action::Start), busy(&ret, "one"));
    }

    #[test]
    fn keep_work_in_progress_across_list_updates() {
        let mut ret = manual();
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

    #[test]
    fn follow_whether_the_client_runs() {
        let running = |ret: &Ret| ret.devices[0].running;
        let done = |action, result: Result<(), &str>| {
            Message::Done("one".to_owned(), action, result.map_err(str::to_owned))
        };
        let mut ret = manual();
        let _ = ret.update(Message::Devices(vec![device("one", "device")]));
        assert_eq!(None, running(&ret));
        let _ = ret.update(Message::ClientRunning("one".to_owned(), false));
        assert_eq!(Some(false), running(&ret));

        let _ = ret.update(done(Action::Start, Ok(())));
        assert_eq!(Some(true), running(&ret));
        // resetting the tunnel or failing to stop leaves the client as it was
        let _ = ret.update(done(Action::Tunnel, Ok(())));
        let _ = ret.update(done(Action::Stop, Err("cannot stop")));
        assert_eq!(Some(true), running(&ret));
        let _ = ret.update(done(Action::Reinstall, Ok(())));
        assert_eq!(Some(false), running(&ret));

        // plugged back: checked again, unless an action finished before the check answered
        let _ = ret.update(Message::Devices(Vec::new()));
        let _ = ret.update(Message::Devices(vec![device("one", "device")]));
        assert_eq!(None, running(&ret));
        let _ = ret.update(Message::Run("one".to_owned(), Action::Start));
        let _ = ret.update(done(Action::Start, Ok(())));
        let _ = ret.update(Message::ClientRunning("one".to_owned(), false));
        assert_eq!(Some(true), running(&ret));
    }

    #[test]
    fn close_the_menu_when_choosing_an_action() {
        let mut ret = manual();
        let _ = ret.update(Message::Devices(vec![device("one", "device")]));
        let _ = ret.update(Message::ToggleMenu("one".to_owned()));
        assert_eq!(Some("one"), ret.menu.as_deref());
        let _ = ret.update(Message::Run("one".to_owned(), Action::Tunnel));
        assert_eq!(None, ret.menu);

        // a device gone takes its open menu along
        let _ = ret.update(Message::ToggleMenu("one".to_owned()));
        let _ = ret.update(Message::Devices(Vec::new()));
        assert_eq!(None, ret.menu);
    }

    #[test]
    fn keep_the_device_info_across_list_updates() {
        let mut ret = manual();
        let _ = ret.update(Message::Devices(vec![device("one", "device")]));
        let quest = DeviceInfo {
            name: Some("Quest 3S".to_owned()),
            headset: true,
        };
        let _ = ret.update(Message::DeviceInfo("one".to_owned(), quest.clone()));
        // another device connecting, then this one losing its authorization
        let _ = ret.update(Message::Devices(vec![
            device("one", "device"),
            device("two", "device"),
        ]));
        let _ = ret.update(Message::Devices(vec![device("one", "unauthorized")]));
        assert_eq!(Some(&quest), ret.devices[0].info.as_ref());

        // an answer for a device gone meanwhile is ignored
        let _ = ret.update(Message::DeviceInfo("two".to_owned(), quest));
        assert_eq!(1, ret.devices.len());
    }

    #[test]
    fn save_the_settings_as_they_change() {
        let dir = std::env::temp_dir().join(format!("ret-ui-app-{}", std::process::id()));
        let path = dir.join("config.toml");
        let (file, config) = ConfigFile::load(path.clone());
        let mut ret = Ret::with_config(config, Some(file));

        let _ = ret.update(Message::PortChanged("4000".to_owned()));
        // an invalid port while typing keeps the last valid one
        let _ = ret.update(Message::PortChanged("40x".to_owned()));
        let _ = ret.update(Message::DnsServersChanged(" 1.1.1.1, ,9.9.9.9 ".to_owned()));
        let _ = ret.update(Message::AutostartToggled(true));

        let (_, saved) = ConfigFile::load(path);
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(
            Config {
                port: 4000,
                dns_servers: vec!["1.1.1.1".to_owned(), "9.9.9.9".to_owned()],
                routes: Vec::new(),
                autostart: true,
                ..Config::default()
            },
            saved
        );
    }

    #[test]
    fn show_the_saved_settings() {
        let config = Config {
            port: 4000,
            dns_servers: vec!["1.1.1.1".to_owned(), "9.9.9.9".to_owned()],
            routes: vec!["10.0.0.0/8".to_owned()],
            autostart: true,
            ..Config::default()
        };
        let ret = Ret::with_config(config, None);
        assert_eq!("4000", ret.port);
        assert_eq!("1.1.1.1,9.9.9.9", ret.dns_servers);
        assert_eq!("10.0.0.0/8", ret.routes);
        assert!(ret.autostart);
    }
}
