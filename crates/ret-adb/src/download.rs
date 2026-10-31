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
    let Some(data_dir) = dirs::data_dir() else {
        return Err(io::Error::other(
            "the data directory of the user is unknown",
        ));
    };
    let tools = data_dir.join("ret");
    let adb = tools.join("platform-tools").join(ADB);
    if adb.is_file() {
        return Ok(adb);
    }
    info!("Downloading the platform tools into {}", tools.display());

    // a partial install cannot be trusted: extract beside the final directory, then move it in
    let staging = tools.join("platform-tools.partial");
    let _ = fs::remove_dir_all(&staging);
    extract(&download()?, &staging)?;
    let _ = fs::remove_dir_all(tools.join("platform-tools"));
    fs::rename(staging.join("platform-tools"), tools.join("platform-tools"))?;
    let _ = fs::remove_dir(&staging);
    Ok(adb)
}

/// Largest archive accepted, in bytes: the platform tools weigh some 16 MB today, and a
/// response beyond this size cannot exhaust the memory
const MAX_ARCHIVE: u64 = 64 * 1024 * 1024;

/// Download the archive of the platform tools
///
/// The archive is read into memory in one piece: `ureq` caps a body at 10 MB by default, which
/// the platform tools already exceed.
fn download() -> io::Result<Vec<u8>> {
    let mut response = ureq::get(PLATFORM_TOOLS)
        .call()
        .map_err(|err| io::Error::other(format!("cannot download the platform tools: {err}")))?;
    response
        .body_mut()
        .with_config()
        .limit(MAX_ARCHIVE)
        .read_to_vec()
        .map_err(|err| io::Error::other(format!("cannot read the platform tools: {err}")))
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
mod tests {
    use super::*;

    use std::io::Write as _;

    use zip::ZipWriter;
    use zip::write::SimpleFileOptions;

    /// An archive with one file per entry, compressed the same way as the platform tools
    fn archive(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut writer = ZipWriter::new(std::io::Cursor::new(Vec::new()));
        for (name, bytes) in entries {
            writer
                .start_file(*name, SimpleFileOptions::default())
                .unwrap();
            writer.write_all(bytes).unwrap();
        }
        writer.finish().unwrap().into_inner()
    }

    #[test]
    fn extracts_the_entries_under_their_directories() {
        let dir = std::env::temp_dir().join(format!("ret-adb-download-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        extract(
            &archive(&[
                ("platform-tools/readme", b"data"),
                ("platform-tools/adb", b"tool"),
            ]),
            &dir,
        )
        .unwrap();
        assert_eq!(
            fs::read(dir.join("platform-tools/readme")).unwrap(),
            b"data"
        );
        assert_eq!(fs::read(dir.join("platform-tools/adb")).unwrap(), b"tool");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn skips_the_entries_which_escape_the_archive() {
        let dir = std::env::temp_dir().join(format!("ret-adb-escape-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        extract(&archive(&[("../escaped", b"data")]), &dir).unwrap();
        assert!(!dir.parent().unwrap().join("escaped").exists());
        let _ = fs::remove_dir_all(&dir);
    }
}
