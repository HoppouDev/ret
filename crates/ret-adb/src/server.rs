//! Lifetime of the ADB server started by the desktop window.

use std::io;
use std::net::{Ipv4Addr, SocketAddr, TcpStream};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, Instant};
use tracing::warn;

#[cfg(windows)]
use std::os::windows::process::CommandExt as _;

const SERVER: SocketAddr = SocketAddr::new(std::net::IpAddr::V4(Ipv4Addr::LOCALHOST), 5037);
const START_TIMEOUT: Duration = Duration::from_secs(5);
static STATE: Mutex<State> = Mutex::new(State {
    managed: false,
    guards: 0,
    child: None,
});

/// Stops the server started by this process when the last guard is dropped.
///
/// Create this before starting ADB work and keep it alive until the window closes.
/// A server which was already running is reused and left alone. Background work
/// cannot start another server once the last guard has been dropped.
#[must_use]
pub struct ServerGuard {
    _private: (),
}

impl ServerGuard {
    pub fn new() -> Self {
        let mut state = STATE.lock().unwrap_or_else(|error| error.into_inner());
        state.managed = true;
        state.guards += 1;
        Self { _private: () }
    }
}

impl Default for ServerGuard {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for ServerGuard {
    fn drop(&mut self) {
        let child = {
            let mut state = STATE.lock().unwrap_or_else(|error| error.into_inner());
            state.guards -= 1;
            if state.guards == 0 {
                state.child.take()
            } else {
                None
            }
        };
        drop(child);
    }
}

struct State {
    managed: bool,
    guards: usize,
    child: Option<OwnedServer>,
}

impl State {
    fn check_open(&self) -> io::Result<()> {
        if self.managed && self.guards == 0 {
            Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "the ADB server is shutting down",
            ))
        } else {
            Ok(())
        }
    }

    fn start_managed(&mut self, command: &mut Command, address: SocketAddr) -> io::Result<()> {
        self.check_open()?;
        if let Some(child) = &mut self.child {
            if child.0.try_wait()?.is_none() {
                return Ok(());
            }
            self.child = None;
        }
        if connect(address).is_ok() {
            return Ok(());
        }
        let mut child = OwnedServer(command.spawn()?);
        let started = Instant::now();
        loop {
            if let Some(status) = child.0.try_wait()? {
                return Err(io::Error::other(format!(
                    "the ADB server exited during startup ({status})"
                )));
            }
            if connect(address).is_ok() {
                self.child = Some(child);
                return Ok(());
            }
            if started.elapsed() >= START_TIMEOUT {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "the ADB server did not start listening",
                ));
            }
            thread::sleep(Duration::from_millis(20));
        }
    }
}

fn connect(address: SocketAddr) -> io::Result<TcpStream> {
    TcpStream::connect_timeout(&address, Duration::from_millis(50))
}

struct OwnedServer(Child);

impl Drop for OwnedServer {
    fn drop(&mut self) {
        if let Ok(Some(_)) = self.0.try_wait() {
            return;
        }
        match self.0.kill() {
            Ok(()) => {
                if let Err(error) = self.0.wait() {
                    warn!("Cannot wait for the ADB server to stop: {error}");
                }
            }
            Err(error) => warn!("Cannot stop the ADB server: {error}"),
        }
    }
}

pub(crate) fn start_server() -> io::Result<()> {
    STATE
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .check_open()?;
    // Downloading can block, so closing the window must not wait for it.
    let mut command = Command::new(adb_program()?);
    #[cfg(windows)]
    command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW avoids a console flash.
    let mut state = STATE.lock().unwrap_or_else(|error| error.into_inner());
    state.check_open()?;
    if state.managed {
        command.args(["server", "nodaemon"]).stdin(Stdio::null());
        state.start_managed(&mut command, SERVER)
    } else {
        let status = command.arg("start-server").status()?;
        if status.success() {
            Ok(())
        } else {
            Err(io::Error::other(format!(
                "`adb start-server` failed ({status})"
            )))
        }
    }
}

fn adb_program() -> io::Result<PathBuf> {
    if let Some(program) = std::env::var_os("ADB") {
        return Ok(program.into());
    }
    crate::download::adb()
}

#[cfg(test)]
mod tests;
