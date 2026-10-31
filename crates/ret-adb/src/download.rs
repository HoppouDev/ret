//! Download of the `adb` program
//!
//! The [platform tools] of Google contain the `adb` server program, as one archive for every
//! supported system. The archive is downloaded and extracted into the data directory of the
//! user, next to the logs of the window. The copy is the only `adb` ret runs: an `adb`
//! installed on the system is never used.
//!
//! [platform tools]: https://developer.android.com/tools/releases/platform-tools

use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

mod progress;

pub use progress::{Progress, subscribe};

static INSTALL: Mutex<()> = Mutex::new(());

use tracing::info;

/// Where the platform tools are downloaded from
#[cfg(windows)]
const PLATFORM_TOOLS: &str =
    "https://dl.google.com/android/repository/platform-tools-latest-windows.zip";
#[cfg(target_os = "macos")]
const PLATFORM_TOOLS: &str =
    "https://dl.google.com/android/repository/platform-tools-latest-darwin.zip";
#[cfg(target_os = "linux")]
const PLATFORM_TOOLS: &str =
    "https://dl.google.com/android/repository/platform-tools-latest-linux.zip";

/// Name of the adb program inside the platform tools
#[cfg(windows)]
const ADB: &str = "adb.exe";
#[cfg(not(windows))]
const ADB: &str = "adb";

/// The path of the `adb` program, downloaded when it is missing
///
/// The platform tools are installed into `<data_dir>/ret/platform-tools`, which `dirs` locates:
/// `%AppData%` on Windows, `~/Library/Application Support` on macOS, `~/.local/share` on Linux.
/// An install which is already there is kept, so that the archive is downloaded once.
pub(crate) fn adb() -> io::Result<PathBuf> {
    let _install = INSTALL.lock().unwrap_or_else(|err| err.into_inner());
    let Some(data_dir) = dirs::data_dir() else {
        let error = io::Error::other("the data directory of the user is unknown");
        progress::publish(Progress::Failed(error.to_string()));
        return Err(error);
    };
    install(&data_dir.join("ret"), download, &mut progress::publish)
}

fn install(
    tools: &Path,
    download: impl FnOnce(&mut dyn FnMut(Progress)) -> io::Result<Vec<u8>>,
    publish: &mut dyn FnMut(Progress),
) -> io::Result<PathBuf> {
    let adb = tools.join("platform-tools").join(ADB);
    if adb.is_file() {
        return Ok(adb);
    }
    info!("Downloading the platform tools into {}", tools.display());
    publish(Progress::Downloading {
        downloaded: 0,
        total: None,
    });
    let result = (|| {
        // A partial install cannot be trusted, so extract beside the final directory.
        let staging = tools.join("platform-tools.partial");
        let _ = fs::remove_dir_all(&staging);
        let archive = download(publish)?;
        publish(Progress::Extracting);
        extract(&archive, &staging)?;
        if !staging.join("platform-tools").join(ADB).is_file() {
            return Err(io::Error::other(
                "the platform tools archive contains no adb program",
            ));
        }
        let _ = fs::remove_dir_all(tools.join("platform-tools"));
        fs::rename(staging.join("platform-tools"), tools.join("platform-tools"))?;
        let _ = fs::remove_dir(&staging);
        Ok(adb)
    })();
    match &result {
        Ok(_) => publish(Progress::Ready),
        Err(error) => publish(Progress::Failed(error.to_string())),
    }
    result
}

/// Largest archive accepted, in bytes: the platform tools weigh some 16 MB today, and a
/// response beyond this size cannot exhaust the memory
const MAX_ARCHIVE: u64 = 64 * 1024 * 1024;

/// Download the archive with bounded reads and throttled byte updates.
fn download(publish: &mut dyn FnMut(Progress)) -> io::Result<Vec<u8>> {
    let mut response = ureq::get(PLATFORM_TOOLS)
        .call()
        .map_err(|err| io::Error::other(format!("cannot download the platform tools: {err}")))?;
    let total = response.body().content_length();
    read_archive(response.body_mut().as_reader(), total, MAX_ARCHIVE, publish)
        .map_err(|err| io::Error::new(err.kind(), format!("cannot read the platform tools: {err}")))
}

fn read_archive(
    mut reader: impl Read,
    total: Option<u64>,
    limit: u64,
    publish: &mut dyn FnMut(Progress),
) -> io::Result<Vec<u8>> {
    let total = total.filter(|total| *total > 0);
    let too_large = || io::Error::other("the platform tools archive exceeds the size limit");
    if total.is_some_and(|total| total > limit) {
        return Err(too_large());
    }
    let mut archive = Vec::with_capacity(total.unwrap_or(0) as usize);
    let mut chunk = [0; 16 * 1024];
    let mut updated = Instant::now();
    publish(Progress::Downloading {
        downloaded: 0,
        total,
    });
    loop {
        // Probe one byte beyond the limit without ever storing an oversized archive.
        let available = (limit - archive.len() as u64).saturating_add(1);
        let size = available.min(chunk.len() as u64) as usize;
        let count = match reader.read(&mut chunk[..size]) {
            Ok(count) => count,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error),
        };
        if count == 0 {
            if total.is_some_and(|total| total != archive.len() as u64) {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "incomplete platform tools archive",
                ));
            }
            publish(Progress::Downloading {
                downloaded: archive.len() as u64,
                total,
            });
            return Ok(archive);
        }
        if count as u64 > limit - archive.len() as u64 {
            return Err(too_large());
        }
        archive.extend_from_slice(&chunk[..count]);
        if updated.elapsed() >= Duration::from_millis(500) {
            publish(Progress::Downloading {
                downloaded: archive.len() as u64,
                total,
            });
            updated = Instant::now();
        }
    }
}

/// Extract the archive of the platform tools into `dir`
///
/// The archive is read from memory, so that a download which fails halfway leaves no partial
/// files behind. Names which escape the archive, such as `../adb`, name no file: they are
/// skipped.
fn extract(archive: &[u8], dir: &Path) -> io::Result<()> {
    let mut archive = zip::ZipArchive::new(io::Cursor::new(archive))
        .map_err(|err| io::Error::other(format!("unreadable platform tools: {err}")))?;
    for index in 0..archive.len() {
        let mut file = archive.by_index(index).map_err(|err| {
            io::Error::other(format!("unreadable entry of the platform tools: {err}"))
        })?;
        let Some(path) = file.enclosed_name() else {
            tracing::warn!(
                "Skip the entry `{}` of the platform tools",
                String::from_utf8_lossy(file.name_raw())
            );
            continue;
        };
        let path = dir.join(path);
        if file.is_dir() {
            fs::create_dir_all(&path)?;
        } else {
            let mut bytes = Vec::new();
            file.read_to_end(&mut bytes)?;
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(&path, bytes)?;
            #[cfg(unix)]
            if let Some(mode) = file.unix_mode() {
                use std::os::unix::fs::PermissionsExt as _;
                fs::set_permissions(&path, fs::Permissions::from_mode(mode))?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
