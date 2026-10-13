mod autorun;
mod cli;
mod logging;

use clap::Parser;
use ret_adb::client::{self, StartOptions};
use ret_adb::{Adb, track};
use std::error::Error;
use std::process::{self, ExitCode};
use tracing::{error, info};

use crate::cli::{Cli, Command};

fn main() -> ExitCode {
    let cli = Cli::parse();
    logging::init();
    match execute(cli.command) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            error!("{err}");
            ExitCode::FAILURE
        }
    }
}

fn execute(command: Command) -> Result<(), Box<dyn Error>> {
    match command {
        Command::Install(device) => client::install(&Adb::from_env(device.serial))?,
        Command::Uninstall(device) => client::uninstall(&Adb::from_env(device.serial))?,
        Command::Reinstall(device) => {
            let adb = Adb::from_env(device.serial);
            client::uninstall(&adb)?;
            client::install(&adb)?;
        }
        Command::Run { device, start } => run(Adb::from_env(device.serial), start.into())?,
        Command::Autorun(start) => autorun::autorun(start.into())?,
        Command::Start { device, start } => {
            client::start(&Adb::from_env(device.serial), &start.into())?
        }
        Command::Autostart(start) => autostart(start.into()),
        Command::Stop(device) => client::stop(&Adb::from_env(device.serial))?,
        Command::Restart { device, start } => {
            let adb = Adb::from_env(device.serial);
            client::stop(&adb)?;
            client::start(&adb, &start.into())?;
        }
        Command::Tunnel { device, port } => {
            client::tunnel(&Adb::from_env(device.serial), port.port)?
        }
        Command::Relay(port) => relay(port.port)?,
    }
    Ok(())
}

/// Start the client and the relay server, stop the client on Ctrl+C
fn run(adb: Adb, options: StartOptions) -> Result<(), Box<dyn Error>> {
    let port = options.port;
    client::spawn_start(adb.clone(), options);
    ctrlc::set_handler(move || {
        info!("Interrupted");
        if let Err(err) = client::stop(&adb) {
            error!("Cannot stop the client: {err}");
        }
        process::exit(0);
    })?;
    relay(port)
}

/// Start the client on every device that connects, forever
fn autostart(options: StartOptions) -> ! {
    let adb = Adb::from_env(None);
    track::watch_devices(&adb, |serial| {
        client::spawn_start(adb.with_serial(serial), options.clone());
    })
}

fn relay(port: u16) -> Result<(), Box<dyn Error>> {
    info!("Starting the relay server on port {port}");
    ret_relay::relay(port).map_err(|err| format!("cannot run the relay server: {err:#}"))?;
    Ok(())
}
