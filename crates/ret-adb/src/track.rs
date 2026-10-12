//! Detection of the devices connected to adb, through the adb server protocol

use std::collections::HashSet;
use std::io::{self, BufReader, Read, Write};
use std::net::{Ipv4Addr, TcpStream};
use std::thread;
use std::time::Duration;
use tracing::{error, info};

use crate::adb::Adb;

const SERVER_PORT: u16 = 5037;

/// Wait before reconnecting after the adb server was started
const RETRY_DELAY: Duration = Duration::from_secs(1);
/// Wait before reconnecting when the adb server could not be started
const RETRY_DELAY_SERVER_DOWN: Duration = Duration::from_secs(5);

/// Call `on_connected` with the serial of every device that becomes available, forever
///
/// The devices already connected count as new. When the adb server cannot be reached, it is
/// started.
pub fn watch_devices(adb: &Adb, mut on_connected: impl FnMut(&str)) -> ! {
    loop {
        // a restarted adb server lost the reverse tunnels, so the devices are started again
        let mut devices = Devices::default();
        let result = TcpStream::connect((Ipv4Addr::LOCALHOST, SERVER_PORT))
            .and_then(|stream| track(stream, &mut devices, &mut on_connected));
        if let Err(err) = result {
            error!("Cannot track the adb devices: {err}");
        }
        info!("Starting the adb server");
        match adb.run(&["start-server"]) {
            Ok(()) => thread::sleep(RETRY_DELAY),
            Err(err) => {
                error!("Cannot start the adb server: {err}");
                thread::sleep(RETRY_DELAY_SERVER_DOWN);
            }
        }
    }
}

/// Follow the device list on a connection to the adb server, until it fails
fn track(
    mut stream: TcpStream,
    devices: &mut Devices,
    on_connected: &mut impl FnMut(&str),
) -> io::Result<()> {
    write_request(&mut stream, "host:track-devices")?;
    let mut reader = BufReader::new(stream);
    read_status(&mut reader)?;
    loop {
        let list = read_message(&mut reader)?;
        for serial in devices.update(&list) {
            on_connected(&serial);
        }
    }
}

fn write_request(stream: &mut impl Write, request: &str) -> io::Result<()> {
    write!(stream, "{:04x}{request}", request.len())
}

/// Read the reply status of a request, `FAIL` comes with a message
fn read_status(reader: &mut impl Read) -> io::Result<()> {
    let mut status = [0u8; 4];
    read_exact(reader, &mut status)?;
    match &status {
        b"OKAY" => Ok(()),
        b"FAIL" => {
            let message = read_message(reader)?;
            Err(io::Error::other(format!("adb server refused: {message}")))
        }
        _ => Err(invalid_data(format!("unexpected status {status:?}"))),
    }
}

/// Read a message prefixed by its length, as 4 hexadecimal digits
fn read_message(reader: &mut impl Read) -> io::Result<String> {
    let mut prefix = [0u8; 4];
    read_exact(reader, &mut prefix)?;
    let length = std::str::from_utf8(&prefix)
        .ok()
        .and_then(|hex| usize::from_str_radix(hex, 16).ok())
        .ok_or_else(|| invalid_data(format!("invalid message length {prefix:?}")))?;
    let mut message = vec![0u8; length];
    read_exact(reader, &mut message)?;
    String::from_utf8(message).map_err(|_| invalid_data("message is not UTF-8".to_owned()))
}

fn read_exact(reader: &mut impl Read, buf: &mut [u8]) -> io::Result<()> {
    reader.read_exact(buf).map_err(|err| {
        if err.kind() == io::ErrorKind::UnexpectedEof {
            io::Error::new(err.kind(), "the adb server closed the connection")
        } else {
            err
        }
    })
}

fn invalid_data(message: String) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

/// The devices available to adb (in the `device` state)
#[derive(Default)]
struct Devices {
    available: HashSet<String>,
}

impl Devices {
    /// Replace the device list by a `track-devices` message, return the devices that became
    /// available, in the order of the message
    fn update(&mut self, list: &str) -> Vec<String> {
        let available: Vec<&str> = list
            .lines()
            .filter_map(|line| {
                let mut fields = line.split_whitespace();
                match (fields.next(), fields.next()) {
                    (Some(serial), Some("device")) => Some(serial),
                    _ => None,
                }
            })
            .collect();
        let new = available
            .iter()
            .filter(|serial| !self.available.contains(**serial))
            .map(|serial| serial.to_string())
            .collect();
        self.available = available.into_iter().map(String::from).collect();
        new
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn write_request_with_its_length() {
        let mut request = Vec::new();
        write_request(&mut request, "host:track-devices").unwrap();
        assert_eq!(b"0012host:track-devices", &request[..]);
    }

    #[test]
    fn read_messages_in_sequence() {
        let mut reader = Cursor::new("OKAY0015emulator-5554\tdevice\n0000");
        read_status(&mut reader).unwrap();
        assert_eq!(
            "emulator-5554\tdevice\n",
            read_message(&mut reader).unwrap()
        );
        assert_eq!("", read_message(&mut reader).unwrap());
        let err = read_message(&mut reader).unwrap_err();
        assert_eq!(io::ErrorKind::UnexpectedEof, err.kind());
    }

    #[test]
    fn report_refusal() {
        let mut reader = Cursor::new("FAIL000funknown command");
        let err = read_status(&mut reader).unwrap_err();
        assert!(err.to_string().contains("unknown command"));
    }

    #[test]
    fn reject_corrupt_length() {
        let mut reader = Cursor::new("zzzzdata");
        let err = read_message(&mut reader).unwrap_err();
        assert_eq!(io::ErrorKind::InvalidData, err.kind());
        let mut reader = Cursor::new("ffff");
        let err = read_message(&mut reader).unwrap_err();
        assert_eq!(io::ErrorKind::UnexpectedEof, err.kind());
    }

    #[test]
    fn report_devices_once_available() {
        let mut devices = Devices::default();
        assert_eq!(
            vec!["0123", "emulator-5554"],
            devices.update("0123\tdevice\nabcd\tunauthorized\nemulator-5554\tdevice\n")
        );
        // authorized meanwhile
        assert_eq!(
            vec!["abcd"],
            devices.update("0123\tdevice\nabcd\tdevice\nemulator-5554\tdevice\n")
        );
        assert!(devices.update("abcd\tdevice\n").is_empty());
        // plugged back
        assert_eq!(vec!["0123"], devices.update("0123\tdevice\nabcd\tdevice\n"));
        assert!(devices.update("").is_empty());
    }
}
