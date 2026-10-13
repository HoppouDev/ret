//! Settings of the window, kept in `<config_dir>/ret/config.toml` between runs

use anyhow::{Context, bail};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use tracing::{error, info, warn};

use crate::app::DEFAULT_PORT;

/// Version of the layout of the configuration file, written in its `version` key
///
/// Increase it when a change cannot be read by older versions of ret (renamed key, changed
/// meaning), and convert the older layouts in [`Config::upgrade`].
pub const VERSION: u32 = 1;

/// The settings saved in the configuration file
///
/// Every field is optional in the file, a missing one takes its default value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Layout of the file, see [`VERSION`]
    ///
    /// Missing in the files written before it existed, which have the layout of version 1.
    pub version: u32,
    /// Port of the relay server on this computer
    pub port: u16,
    /// DNS servers used by the devices, empty for the client default (8.8.8.8)
    pub dns_servers: Vec<String>,
    /// Routes to reverse tether, empty for the whole traffic (0.0.0.0/0)
    pub routes: Vec<String>,
    /// Whether the client starts on every device as soon as adb sees it
    pub autostart: bool,
    /// Colors of the window
    pub theme: ThemeChoice,
}

/// The colors chosen for the window
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemeChoice {
    /// Light or dark like the system, following its changes
    #[default]
    System,
    Light,
    Dark,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            version: VERSION,
            port: DEFAULT_PORT,
            dns_servers: Vec::new(),
            routes: Vec::new(),
            autostart: true,
            theme: ThemeChoice::System,
        }
    }
}

impl Config {
    /// Convert a configuration read from a file of an older layout to the current one
    ///
    /// A file written by a newer version of ret is refused: it may hold settings this version
    /// would misread, and saving would lose them.
    fn upgrade(self) -> anyhow::Result<Self> {
        if self.version > VERSION {
            bail!(
                "the settings are of version {}, written by a newer ret (this one reads up to {VERSION})",
                self.version
            );
        }
        // version 1 is the first layout, there is nothing to convert yet
        Ok(Self {
            version: VERSION,
            ..self
        })
    }
}

/// Where the configuration is loaded from and saved to
#[derive(Debug)]
pub struct ConfigFile {
    path: PathBuf,
    /// False when the file could not be read: saving would overwrite what the user wrote
    writable: bool,
    /// The keys of the file this version does not know, written back as they were
    ///
    /// They may be settings of a newer version of ret, or typos which the user can then fix.
    unknown: toml::Table,
}

impl ConfigFile {
    /// The file in the configuration directory of the user, if the system has one
    pub fn default_path() -> Option<PathBuf> {
        dirs::config_dir().map(|dir| dir.join("ret").join("config.toml"))
    }

    /// Load the configuration at `path`, creating the file with the defaults if it does not
    /// exist yet, so that it can be edited by hand before changing anything in the window
    ///
    /// If the file cannot be read or parsed, the defaults are used and the file is never
    /// overwritten, so that a typo does not lose the other settings.
    pub fn load(path: PathBuf) -> (Self, Config) {
        match read(&path) {
            Ok(Some((config, unknown))) => {
                info!("Settings loaded from {}", path.display());
                for key in unknown.keys() {
                    warn!(
                        "Unknown setting `{key}` in {}, kept but not used",
                        path.display()
                    );
                }
                let file = Self {
                    path,
                    writable: true,
                    unknown,
                };
                (file, config)
            }
            Ok(None) => {
                let file = Self {
                    path,
                    writable: true,
                    unknown: toml::Table::new(),
                };
                let config = Config::default();
                if file.save(&config) {
                    info!("Default settings written to {}", file.path.display());
                }
                (file, config)
            }
            Err(err) => {
                error!("{err:#}, using the default settings without saving them");
                let file = Self {
                    path,
                    writable: false,
                    unknown: toml::Table::new(),
                };
                (file, Config::default())
            }
        }
    }

    /// Save `config`, logging the error if it cannot be written, return whether it was
    pub fn save(&self, config: &Config) -> bool {
        if !self.writable {
            return false;
        }
        match write(&self.path, config, &self.unknown) {
            Ok(()) => true,
            Err(err) => {
                error!("{err:#}");
                false
            }
        }
    }
}

/// The configuration at `path` and its unknown keys, `None` if there is no such file
fn read(path: &Path) -> anyhow::Result<Option<(Config, toml::Table)>> {
    let content = match fs::read_to_string(path) {
        Ok(content) => content,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(err) => {
            return Err(err).with_context(|| format!("cannot read {}", path.display()));
        }
    };
    let mut table: toml::Table = toml::from_str(&content)
        .with_context(|| format!("invalid settings in {}", path.display()))?;
    let config = Config::deserialize(table.clone())
        .with_context(|| format!("invalid settings in {}", path.display()))?
        .upgrade()
        .with_context(|| format!("cannot use the settings in {}", path.display()))?;
    let known = toml::Table::try_from(&config).context("cannot serialize the settings")?;
    table.retain(|key, _| !known.contains_key(key));
    Ok(Some((config, table)))
}

/// Write `config` then the `unknown` keys to `path`, through a temporary file so that a crash
/// cannot leave it truncated
fn write(path: &Path, config: &Config, unknown: &toml::Table) -> anyhow::Result<()> {
    let mut content = toml::to_string_pretty(config).context("cannot serialize the settings")?;
    // after the known keys, which are all plain values: tables among the unknown keys stay last
    content.push_str(&toml::to_string_pretty(unknown).context("cannot serialize the settings")?);
    let dir = path
        .parent()
        .context("the settings file has no parent directory")?;
    fs::create_dir_all(dir).with_context(|| format!("cannot create {}", dir.display()))?;
    let temporary = path.with_extension("toml.tmp");
    fs::write(&temporary, content)
        .and_then(|()| fs::rename(&temporary, path))
        .with_context(|| format!("cannot save the settings to {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fresh directory under the system temporary directory, removed when dropped
    struct TempDir(PathBuf);

    impl TempDir {
        fn new(name: &str) -> Self {
            let dir = std::env::temp_dir().join(format!("ret-ui-{name}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&dir);
            Self(dir)
        }

        fn file(&self) -> PathBuf {
            self.0.join("ret").join("config.toml")
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn create_the_file_with_the_defaults() {
        let dir = TempDir::new("missing");
        let (_, config) = ConfigFile::load(dir.file());
        assert_eq!(Config::default(), config);
        assert_eq!(31416, config.port);

        // the file exists right away, with every setting to edit
        let written = fs::read_to_string(dir.file()).unwrap();
        assert_eq!(Config::default(), toml::from_str(&written).unwrap());
    }

    #[test]
    fn save_then_load() {
        let dir = TempDir::new("roundtrip");
        let config = Config {
            port: 4000,
            dns_servers: vec!["1.1.1.1".to_owned(), "9.9.9.9".to_owned()],
            routes: vec!["10.0.0.0/8".to_owned()],
            autostart: true,
            ..Config::default()
        };
        let (file, _) = ConfigFile::load(dir.file());
        // the directory is created on the first save
        file.save(&config);

        let (_, loaded) = ConfigFile::load(dir.file());
        assert_eq!(config, loaded);
        let saved = fs::read_to_string(dir.file()).unwrap();
        assert!(saved.starts_with("version = 1\n"), "{saved}");
    }

    #[test]
    fn read_files_without_version_as_the_first_layout() {
        let dir = TempDir::new("unversioned");
        fs::create_dir_all(dir.file().parent().unwrap()).unwrap();
        fs::write(dir.file(), "port = 4000\n").unwrap();

        let (_, config) = ConfigFile::load(dir.file());
        assert_eq!(1, config.version);
        assert_eq!(4000, config.port);
    }

    #[test]
    fn keep_a_file_of_a_newer_version() {
        let dir = TempDir::new("newer");
        fs::create_dir_all(dir.file().parent().unwrap()).unwrap();
        let content = "version = 2\nport = 4000\nfuture_setting = true\n";
        fs::write(dir.file(), content).unwrap();

        let (file, config) = ConfigFile::load(dir.file());
        assert_eq!(Config::default(), config);

        // saving must not lose the settings this version does not know
        file.save(&Config::default());
        assert_eq!(content, fs::read_to_string(dir.file()).unwrap());
    }

    #[test]
    fn missing_fields_take_their_default() {
        let dir = TempDir::new("partial");
        fs::create_dir_all(dir.file().parent().unwrap()).unwrap();
        fs::write(dir.file(), "autostart = false\ntheme = \"light\"\n").unwrap();

        let (_, config) = ConfigFile::load(dir.file());
        assert_eq!(
            Config {
                autostart: false,
                theme: ThemeChoice::Light,
                ..Config::default()
            },
            config
        );
    }

    #[test]
    fn keep_unknown_keys_when_saving() {
        let dir = TempDir::new("unknown");
        fs::create_dir_all(dir.file().parent().unwrap()).unwrap();
        // a typo, and a setting of a newer ret which kept the same layout
        let content = "autostrat = false\nport = 4000\n[window]\nwidth = 800\n";
        fs::write(dir.file(), content).unwrap();

        let (file, config) = ConfigFile::load(dir.file());
        assert_eq!(4000, config.port);
        assert!(file.save(&Config {
            port: 5000,
            ..config
        }));

        let saved: toml::Table = toml::from_str(&fs::read_to_string(dir.file()).unwrap()).unwrap();
        assert_eq!(Some(&toml::Value::Integer(5000)), saved.get("port"));
        assert_eq!(Some(&toml::Value::Boolean(false)), saved.get("autostrat"));
        assert_eq!(
            Some(&toml::Value::Integer(800)),
            saved.get("window").and_then(|window| window.get("width"))
        );
    }

    #[test]
    fn keep_an_invalid_file() {
        let dir = TempDir::new("invalid");
        fs::create_dir_all(dir.file().parent().unwrap()).unwrap();
        let content = "port = \"not a number\"\nautostart = true\n";
        fs::write(dir.file(), content).unwrap();

        let (file, config) = ConfigFile::load(dir.file());
        assert_eq!(Config::default(), config);

        // saving must not replace what the user wrote
        file.save(&Config::default());
        assert_eq!(content, fs::read_to_string(dir.file()).unwrap());
    }
}
