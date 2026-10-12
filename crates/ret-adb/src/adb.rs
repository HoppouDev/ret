use std::ffi::OsString;
use std::fmt;
use std::io;
use std::process::{Command, ExitStatus, Output};
use tracing::debug;

/// An `adb` invocation that failed
#[derive(Debug)]
pub enum Error {
    /// `adb` could not be executed at all
    Spawn { command: String, source: io::Error },
    /// `adb` ran and reported a failure
    Status { command: String, status: ExitStatus },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::Spawn { command, source } => write!(f, "cannot execute `{command}`: {source}"),
            Error::Status { command, status } => write!(f, "`{command}` failed ({status})"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Spawn { source, .. } => Some(source),
            Error::Status { .. } => None,
        }
    }
}

/// Runs `adb` commands, against one device when a serial is set
#[derive(Debug, Clone)]
pub struct Adb {
    program: OsString,
    serial: Option<String>,
}

impl Adb {
    /// Use the `adb` from the `ADB` environment variable, or the one in `PATH`
    pub fn from_env(serial: Option<String>) -> Self {
        Self {
            program: std::env::var_os("ADB").unwrap_or_else(|| "adb".into()),
            serial,
        }
    }

    /// The same `adb`, targeting another device
    pub fn with_serial(&self, serial: &str) -> Self {
        Self {
            program: self.program.clone(),
            serial: Some(serial.to_owned()),
        }
    }

    /// Run a command, its output goes to the terminal
    pub fn run(&self, args: &[&str]) -> Result<(), Error> {
        let mut command = self.command(args);
        let status = command.status().map_err(|source| Error::Spawn {
            command: describe(&command),
            source,
        })?;
        check(&command, status)
    }

    /// Run a command and capture its standard output
    pub fn output(&self, args: &[&str]) -> Result<String, Error> {
        let mut command = self.command(args);
        let Output { status, stdout, .. } = command.output().map_err(|source| Error::Spawn {
            command: describe(&command),
            source,
        })?;
        check(&command, status)?;
        Ok(String::from_utf8_lossy(&stdout).into_owned())
    }

    fn command(&self, args: &[&str]) -> Command {
        let mut command = Command::new(&self.program);
        if let Some(serial) = &self.serial {
            command.args(["-s", serial]);
        }
        command.args(args);
        debug!("Execute `{}`", describe(&command));
        command
    }
}

fn check(command: &Command, status: ExitStatus) -> Result<(), Error> {
    if status.success() {
        Ok(())
    } else {
        Err(Error::Status {
            command: describe(command),
            status,
        })
    }
}

fn describe(command: &Command) -> String {
    let mut description = command.get_program().to_string_lossy().into_owned();
    for arg in command.get_args() {
        description.push(' ');
        description.push_str(&arg.to_string_lossy());
    }
    description
}
