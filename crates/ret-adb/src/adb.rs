//! Device commands, sent to the adb server through the `adb_client` library
//!
//! The `adb` program is not involved: the commands run directly against the adb server on
//! `127.0.0.1:5037`, which forwards them to the device. Only starting the server itself (see
//! `track`) still runs the `adb` program.

use std::ffi::OsStr;
use std::fmt;
use std::fs::File;
use std::io::{self, Write};
use std::path::Path;
use std::sync::mpsc;

use adb_client::server_device::ADBServerDevice;
use adb_client::{ADBDeviceExt, RustADBError};
use tracing::debug;

/// A device command that failed
#[derive(Debug)]
pub enum Error {
    /// The adb server or the connection to the device failed
    Client(RustADBError),
    /// A command on the device ended with a failure status
    Status { command: String, message: String },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::Client(source) => write!(f, "{source}"),
            Error::Status { command, message } if message.is_empty() => {
                write!(f, "`{command}` failed")
            }
            Error::Status { command, message } => write!(f, "`{command}` failed: {message}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Client(source) => Some(source),
            Error::Status { .. } => None,
        }
    }
}

impl From<io::Error> for Error {
    fn from(source: io::Error) -> Self {
        Error::Client(RustADBError::IOError(source))
    }
}

/// Sends commands to one device through the adb server
#[derive(Debug, Clone)]
pub struct Adb {
    serial: Option<String>,
}

/// The line closing the output of device commands, telling their exit status
///
/// `adb_client` cannot report the status of a shell command: it drops the packet of the shell
/// protocol when the stream closes. The commands sent by `Adb::shell` print their status
/// instead, as the last line of their output.
const EXIT_MARKER: &str = "__ret_exit=";

impl Adb {
    /// Control the device `serial`, or the only device adb knows when none is given
    pub fn new(serial: Option<String>) -> Self {
        Self { serial }
    }

    /// The same adb server, targeting another device
    pub fn with_serial(&self, serial: &str) -> Self {
        Self {
            serial: Some(serial.to_owned()),
        }
    }

    /// Run a command in the shell of the device and return its standard output
    pub fn output(&self, command: &str) -> Result<String, Error> {
        self.shell(command, false)
    }

    /// Run a command in the shell of the device, its output goes to the terminal
    pub fn run(&self, command: &str) -> Result<(), Error> {
        self.shell(command, true).map(|_| ())
    }

    /// Run a command in the shell of the device, checking its exit status
    ///
    /// `adb_client` drops the exit status of the shell protocol, so the command reports it
    /// itself: `echo` closes the output with `EXIT_MARKER`. The status line is stripped from
    /// the output, which is returned, and when `terminal` is set the output goes to the
    /// terminal as the `adb` program did, with the errors of the device on the error stream.
    fn shell(&self, command: &str, terminal: bool) -> Result<String, Error> {
        debug!("Run on the device `{command}`");
        let mut stdout = Vec::new();
        let stderr: &mut dyn io::Write = if terminal {
            &mut io::stderr()
        } else {
            &mut io::sink()
        };
        let mut device = self.device();
        device
            .shell_command(
                &format!("{command}; echo {EXIT_MARKER}$?"),
                Some(&mut stdout),
                Some(stderr),
            )
            .map_err(Error::Client)?;
        let stdout = String::from_utf8_lossy(&stdout).into_owned();
        let (body, status) = stdout
            .rsplit_once(EXIT_MARKER)
            .ok_or_else(|| Error::Status {
                command: command.to_owned(),
                message: "the device did not report the status".to_owned(),
            })?;
        let code: u8 = status.trim().parse().map_err(|_| Error::Status {
            command: command.to_owned(),
            message: format!("unreadable status `{}`", status.trim()),
        })?;
        if terminal {
            io::stdout().write_all(body.as_bytes())?;
        }
        if code == 0 {
            Ok(body.to_owned())
        } else {
            Err(Error::Status {
                command: command.to_owned(),
                message: format!("exit code {code}"),
            })
        }
    }

    /// Install the apk file, replacing the version already on the device
    ///
    /// The device reads the apk from the standard input of its `cmd package install`, like
    /// `adb install -r` does.
    pub fn install(&self, apk: &OsStr) -> Result<(), Error> {
        let path = Path::new(apk);
        let mut file = File::open(path)?;
        let size = file.metadata()?.len();
        let command = format!("cmd package install -r -S {size}");
        debug!("Install `{}` ({size} bytes)", path.display());

        // the session reports its result only once the command ends, so its output is collected
        // until the adb server closes the stream
        let (sender, receiver) = mpsc::channel();
        let mut device = self.device();
        let session = device.exec(&command, &mut file, Box::new(Forward(sender)));
        let mut reply = Vec::new();
        while let Ok(chunk) = receiver.recv() {
            reply.extend_from_slice(&chunk);
        }
        session.map_err(Error::Client)?;

        if reply.starts_with(b"Success") {
            Ok(())
        } else {
            let message = String::from_utf8_lossy(&reply).trim().to_owned();
            Err(Error::Status {
                command: format!("install {}", path.display()),
                message,
            })
        }
    }

    /// Remove the package from the device
    pub fn uninstall(&self, package: &str) -> Result<(), Error> {
        debug!("Uninstall `{package}` from the device");
        self.device()
            .uninstall(package, None)
            .map_err(Error::Client)
    }

    /// Ask the adb server to open the connections to `remote` on the device into `local`
    pub fn reverse(&self, remote: &str, local: &str) -> Result<(), Error> {
        debug!("Reverse `{remote}` into `{local}`");
        self.device()
            .reverse(remote.to_owned(), local.to_owned())
            .map_err(Error::Client)
    }

    /// A connection to the device through the adb server
    fn device(&self) -> ADBServerDevice {
        match &self.serial {
            Some(serial) => ADBServerDevice::new(serial.clone(), None),
            None => ADBServerDevice::autodetect(None),
        }
    }
}

/// Sends the output of a device command through a channel
///
/// The reader of the session owns the sender and keeps it until the adb server closes the
/// stream, which happens once the command on the device ends.
struct Forward(mpsc::Sender<Vec<u8>>);

impl io::Write for Forward {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let _ = self.0.send(buf.to_vec());
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
