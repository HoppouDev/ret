use clap::builder::styling::{AnsiColor, Effects, Styles};
use clap::{Args, Parser, Subcommand, value_parser};
use ret_adb::client::StartOptions;

/// Port the relay server listens on when none is given
pub const DEFAULT_PORT: u16 = 31416;

/// Colors of the help and error messages, the same palette as cargo
///
/// The terminal theme picks the actual colors, and they are dropped when the output is not a
/// terminal or `NO_COLOR` is set.
const STYLES: Styles = Styles::styled()
    .header(AnsiColor::Green.on_default().effects(Effects::BOLD))
    .usage(AnsiColor::Green.on_default().effects(Effects::BOLD))
    .literal(AnsiColor::Cyan.on_default().effects(Effects::BOLD))
    .placeholder(AnsiColor::Cyan.on_default())
    .error(AnsiColor::Red.on_default().effects(Effects::BOLD))
    .valid(AnsiColor::Cyan.on_default().effects(Effects::BOLD))
    .invalid(AnsiColor::Yellow.on_default().effects(Effects::BOLD));

/// Reverse tethering for Android over adb
#[derive(Debug, Parser)]
#[command(name = "ret", version, arg_required_else_help = true, styles = STYLES)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Install the client on the device
    Install(DeviceArgs),

    /// Uninstall the client from the device
    Uninstall(DeviceArgs),

    /// Uninstall then install the client
    Reinstall(DeviceArgs),

    /// Reverse tether one device until Ctrl+C
    ///
    /// Installs the client if needed, starts it and runs the relay server. On Ctrl+C, the client
    /// is stopped along with the relay server.
    Run {
        #[command(flatten)]
        device: DeviceArgs,
        #[command(flatten)]
        start: StartArgs,
    },

    /// Reverse tether every device, present and future
    ///
    /// Starts the client on each device adb detects. The relay server runs while a client runs,
    /// and stops when none does (the clients are checked every few seconds).
    Autorun(StartArgs),

    /// Start the client on the device
    ///
    /// Installs the client if needed. If it is already started, nothing happens and the options
    /// are ignored. On the device, 10.0.2.2 reaches localhost on this computer.
    Start {
        #[command(flatten)]
        device: DeviceArgs,
        #[command(flatten)]
        start: StartArgs,
    },

    /// Start the client on every device adb detects, present and future
    Autostart(StartArgs),

    /// Stop the client on the device
    Stop(DeviceArgs),

    /// Stop then start the client
    Restart {
        #[command(flatten)]
        device: DeviceArgs,
        #[command(flatten)]
        start: StartArgs,
    },

    /// Set up the adb reverse tunnel
    ///
    /// Resetting the tunnel brings the connection back after the device was unplugged and plugged
    /// back in.
    Tunnel {
        #[command(flatten)]
        device: DeviceArgs,
        #[command(flatten)]
        port: PortArgs,
    },

    /// Run the relay server in the foreground
    Relay(PortArgs),
}

#[derive(Debug, Args)]
pub struct DeviceArgs {
    /// Serial of the device, required when adb sees several devices
    pub serial: Option<String>,
}

#[derive(Debug, Clone, Args)]
pub struct StartArgs {
    /// DNS servers the device uses, comma-separated [default: 8.8.8.8]
    #[arg(short, long, value_name = "DNS[,DNS...]")]
    pub dns_servers: Option<String>,

    /// Only reverse tether these routes, comma-separated [default: 0.0.0.0/0]
    #[arg(short, long, value_name = "ROUTE[,ROUTE...]")]
    pub routes: Option<String>,

    #[command(flatten)]
    pub port: PortArgs,
}

impl From<StartArgs> for StartOptions {
    fn from(args: StartArgs) -> Self {
        Self {
            dns_servers: args.dns_servers,
            routes: args.routes,
            port: args.port.port,
        }
    }
}

#[derive(Debug, Clone, Copy, Args)]
pub struct PortArgs {
    /// Port of the relay server
    #[arg(
        short,
        long,
        default_value_t = DEFAULT_PORT,
        value_parser = value_parser!(u16).range(1..),
    )]
    pub port: u16,
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;
    use clap::error::ErrorKind;

    fn parse(args: &[&str]) -> Result<Command, clap::Error> {
        Cli::try_parse_from(["ret"].iter().chain(args)).map(|cli| cli.command)
    }

    #[test]
    fn definition_is_valid() {
        Cli::command().debug_assert();
    }

    #[test]
    fn options_in_any_order() {
        let Command::Run { device, start } = parse(&[
            "run",
            "-d",
            "1.1.1.1,9.9.9.9",
            "myserial",
            "-p",
            "1234",
            "-r",
            "10.0.0.0/8",
        ])
        .unwrap() else {
            panic!("Not the run command");
        };
        assert_eq!(Some("myserial"), device.serial.as_deref());
        assert_eq!(Some("1.1.1.1,9.9.9.9"), start.dns_servers.as_deref());
        assert_eq!(Some("10.0.0.0/8"), start.routes.as_deref());
        assert_eq!(1234, start.port.port);
    }

    #[test]
    fn defaults() {
        let Command::Start { device, start } = parse(&["start"]).unwrap() else {
            panic!("Not the start command");
        };
        assert_eq!(None, device.serial);
        assert_eq!(None, start.dns_servers);
        assert_eq!(None, start.routes);
        assert_eq!(DEFAULT_PORT, start.port.port);
    }

    #[test]
    fn reject_port_zero() {
        let err = parse(&["relay", "-p", "0"]).unwrap_err();
        assert_eq!(ErrorKind::ValueValidation, err.kind());
    }

    #[test]
    fn reject_repeated_option() {
        let err = parse(&["start", "-d", "8.8.8.8", "-d", "1.1.1.1"]).unwrap_err();
        assert_eq!(ErrorKind::ArgumentConflict, err.kind());
    }

    #[test]
    fn reject_options_a_command_does_not_take() {
        let err = parse(&["relay", "myserial"]).unwrap_err();
        assert_eq!(ErrorKind::UnknownArgument, err.kind());
        let err = parse(&["autostart", "myserial"]).unwrap_err();
        assert_eq!(ErrorKind::UnknownArgument, err.kind());
        let err = parse(&["stop", "-p", "1234"]).unwrap_err();
        assert_eq!(ErrorKind::UnknownArgument, err.kind());
    }

    #[test]
    fn reject_second_serial() {
        let err = parse(&["install", "one", "two"]).unwrap_err();
        assert_eq!(ErrorKind::UnknownArgument, err.kind());
    }
}
