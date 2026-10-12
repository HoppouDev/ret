//! Control of the ret Android app (the client) through adb

use std::ffi::OsString;
use std::thread;
use std::time::Duration;
use tracing::{error, info};

use crate::adb::{Adb, Error};

/// How the client is configured when it starts
#[derive(Debug, Clone)]
pub struct StartOptions {
    /// DNS servers of the device, comma-separated (the client uses 8.8.8.8 when unset)
    pub dns_servers: Option<String>,
    /// Routes to reverse tether, comma-separated (the client uses 0.0.0.0/0 when unset)
    pub routes: Option<String>,
    /// Port of the relay server on this computer
    pub port: u16,
}

const PACKAGE: &str = "dev.hoppou.ret";
const ACTIVITY: &str = "dev.hoppou.ret/.RetActivity";

/// `versionCode` of the app this program works with, derived from the version of the app the same
/// way as `versionCodeOf()` in `app/build.gradle.kts`
///
/// `cog bump` gives the same version to the app and to the crates.
const VERSION_CODE: u32 = version_code(
    env!("CARGO_PKG_VERSION_MAJOR"),
    env!("CARGO_PKG_VERSION_MINOR"),
    env!("CARGO_PKG_VERSION_PATCH"),
);

/// Name of the abstract socket the app connects to on the device
const SOCKET_NAME: &str = "ret";

/// Delay after an install, so that the app actions are registered before they are started
const INSTALL_SETTLE_DELAY: Duration = Duration::from_millis(500);

/// Path of the apk, from the `RET_APK` environment variable or `ret.apk`
fn apk_path() -> OsString {
    std::env::var_os("RET_APK").unwrap_or_else(|| "ret.apk".into())
}

pub fn install(adb: &Adb) -> Result<(), Error> {
    info!("Installing the client");
    let apk = apk_path();
    adb.run(&["install", "-r", &apk.to_string_lossy()])
}

pub fn uninstall(adb: &Adb) -> Result<(), Error> {
    info!("Uninstalling the client");
    adb.run(&["uninstall", PACKAGE])
}

/// Install the client if it is missing or another version, then start it
pub fn start(adb: &Adb, options: &StartOptions) -> Result<(), Error> {
    info!("Checking the client");
    let dumpsys = adb.output(&["shell", "dumpsys", "package", PACKAGE])?;
    let installed = installed_version_code(&dumpsys).and_then(|code| code.parse().ok());
    if installed != Some(VERSION_CODE) {
        install(adb)?;
        thread::sleep(INSTALL_SETTLE_DELAY);
    }

    info!("Starting the client");
    tunnel(adb, options.port)?;
    let mut args = vec![
        "shell",
        "am",
        "start",
        "-a",
        "dev.hoppou.ret.START",
        "-n",
        ACTIVITY,
    ];
    if let Some(dns_servers) = &options.dns_servers {
        args.extend(["--esa", "dnsServers", dns_servers]);
    }
    if let Some(routes) = &options.routes {
        args.extend(["--esa", "routes", routes]);
    }
    adb.run(&args)
}

/// Start the client in the background, so that the relay server can start meanwhile
pub fn spawn_start(adb: Adb, options: StartOptions) {
    thread::spawn(move || {
        if let Err(err) = start(&adb, &options) {
            error!("Cannot start the client: {err}");
        }
    });
}

pub fn stop(adb: &Adb) -> Result<(), Error> {
    info!("Stopping the client");
    adb.run(&[
        "shell",
        "am",
        "start",
        "-a",
        "dev.hoppou.ret.STOP",
        "-n",
        ACTIVITY,
    ])
}

/// Forward the connections of the client to the relay server
pub fn tunnel(adb: &Adb, port: u16) -> Result<(), Error> {
    adb.run(&[
        "reverse",
        &format!("localabstract:{SOCKET_NAME}"),
        &format!("tcp:{port}"),
    ])
}

/// Extract the `versionCode` of the installed package from `dumpsys package` output
fn installed_version_code(dumpsys: &str) -> Option<&str> {
    dumpsys
        .split_whitespace()
        .find_map(|token| token.strip_prefix("versionCode="))
}

/// 10000 * major + 100 * minor + patch, from the version components given by Cargo
const fn version_code(major: &str, minor: &str, patch: &str) -> u32 {
    let (minor, patch) = (parse(minor), parse(patch));
    assert!(
        minor < 100 && patch < 100,
        "minor and patch must be below 100"
    );
    10000 * parse(major) + 100 * minor + patch
}

const fn parse(number: &str) -> u32 {
    let digits = number.as_bytes();
    let mut value = 0;
    let mut i = 0;
    while i < digits.len() {
        assert!(digits[i].is_ascii_digit(), "not a number");
        value = value * 10 + (digits[i] - b'0') as u32;
        i += 1;
    }
    value
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_version_code_of_installed_package() {
        let dumpsys = "Packages:\n  Package [dev.hoppou.ret] (4c1d2e):\n    \
                       userId=10190\n    pkg=Package{8a3f dev.hoppou.ret}\n    \
                       versionCode=9 minSdk=21 targetSdk=36\n    versionName=2.5.1\n";
        assert_eq!(Some("9"), installed_version_code(dumpsys));
    }

    #[test]
    fn no_version_code_when_not_installed() {
        let dumpsys = "Dexopt state:\n  Unable to find package: dev.hoppou.ret\n";
        assert_eq!(None, installed_version_code(dumpsys));
    }

    #[test]
    fn version_code_matches_the_app_formula() {
        // same values as versionCodeOf() in app/build.gradle.kts
        assert_eq!(30002, version_code("3", "0", "2"));
        assert_eq!(31099, version_code("3", "10", "99"));
        assert_eq!(100000, version_code("10", "0", "0"));
    }
}
