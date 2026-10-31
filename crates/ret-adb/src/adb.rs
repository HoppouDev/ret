//! Device commands, sent to the adb server through the `adb_client` library
//!
//! The `adb` program is not involved: the commands run directly against the adb server on
//! `127.0.0.1:5037`, which forwards them to the device. Only starting that server still runs
//! the `adb` program (see [`start_server`]), which happens when a command cannot reach it and
//! while tracking the devices.

use std::ffi::OsStr;
use std::fmt;
use std::fs::File;
use std::io::{self, Read, Write};
use std::path::Path;
use std::sync::mpsc;

use adb_client::server_device::ADBServerDevice;
use adb_client::{ADBDeviceExt, RustADBError};
use tracing::debug;

use crate::server::start_server;

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

/// The output suffix telling the exit status of a device command
///
/// `adb_client` cannot report the status of a shell command: it drops the packet of the shell
/// protocol when the stream closes. The commands sent by `Adb::shell` append the status of the
/// command after `EXIT_MARKER` instead. `echo` prints no newline of its own, so the suffix
/// sticks to the last line of the output when the command does not end with one.
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
    /// itself: `echo` closes the output with `EXIT_MARKER` as a suffix. The status is stripped
    /// from the output, which is returned, and when `terminal` is set the output goes to the
    /// terminal as the `adb` program did, with the errors of the device on the error stream.
    fn shell(&self, command: &str, terminal: bool) -> Result<String, Error> {
        debug!("Run on the device `{command}`");
        let marked = format!("{command}; echo {EXIT_MARKER}$?");
        let mut stdout = Vec::new();
        let stderr: &mut dyn io::Write = if terminal {
            &mut io::stderr()
        } else {
            &mut io::sink()
        };
        self.with_device(|device| {
            device.shell_command(&marked, Some(&mut stdout), Some(&mut *stderr))
        })?;
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
        let mut buffer = Vec::new();
        File::open(path)?.read_to_end(&mut buffer)?;
        let size = buffer.len();
        let command = format!("cmd package install -r -S {size}");
        debug!("Install `{}` ({size} bytes)", path.display());

        let (device, session, receiver) = self.install_stream(&command, &mut buffer);
        // the stream is only unreachable when the adb server does not run: start it, then try
        // once more
        let retry = match &session {
            Err(RustADBError::IOError(err)) => err.kind() == io::ErrorKind::ConnectionRefused,
            _ => false,
        };
        let (device, session, receiver) = if retry {
            start_server()?;
            self.install_stream(&command, &mut buffer)
        } else {
            (device, session, receiver)
        };
        // the session reports its result only once the command ends, so its output is collected
        // until the adb server closes the stream
        let mut reply = Vec::new();
        if session.is_err() {
            // the copy of the apk failed: the command on the device waits for bytes which never
            // come, so close the connection, which ends the reader and closes the channel
            drop(device);
        }
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
        self.with_device(|device| device.uninstall(package, None))
    }

    /// Ask the adb server to open the connections to `remote` on the device into `local`
    pub fn reverse(&self, remote: &str, local: &str) -> Result<(), Error> {
        debug!("Reverse `{remote}` into `{local}`");
        self.with_device(|device| device.reverse(remote.to_owned(), local.to_owned()))
    }

    /// Run `operation` against the device through a fresh connection
    ///
    /// When the connection is refused, the adb server does not run: start it as the `adb`
    /// program did, then try once more.
    fn with_device<T>(
        &self,
        mut operation: impl FnMut(&mut ADBServerDevice) -> Result<T, RustADBError>,
    ) -> Result<T, Error> {
        let mut device = self.device();
        match operation(&mut device) {
            Err(RustADBError::IOError(err)) if err.kind() == io::ErrorKind::ConnectionRefused => {
                start_server()?;
                operation(&mut device).map_err(Error::Client)
            }
            result => result.map_err(Error::Client),
        }
    }

    /// Stream the apk into the device, with a reader thread collecting its output into a
    /// channel
    ///
    /// The device must stay alive until the adb server closes the stream, which happens once
    /// the command on the device ends.
    fn install_stream(
        &self,
        command: &str,
        buffer: &mut Vec<u8>,
    ) -> (
        ADBServerDevice,
        Result<(), RustADBError>,
        mpsc::Receiver<Vec<u8>>,
    ) {
        let (sender, receiver) = mpsc::channel();
        let mut device = self.device();
        let session = device.exec(command, &mut buffer.as_slice(), Box::new(Forward(sender)));
        (device, session, receiver)
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
