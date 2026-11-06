//! Server configuration, layered from defaults, a TOML file and `NAMA_*` environment variables.

use std::{
    collections::BTreeMap,
    net::SocketAddr,
    path::{Path, PathBuf},
};

use figment::{
    Figment,
    providers::{Env, Format, Serialized, Toml},
};
use serde::{Deserialize, Serialize};

/// The config file read from the working directory when no file is named.
pub const DEFAULT_FILE: &str = "nama.toml";

/// Prefix of the environment variables that override the config file, e.g. `NAMA_DATA_DIR`.
pub const ENV_PREFIX: &str = "NAMA_";

/// Environment variable naming the config file. It picks the file rather than setting a key, so the
/// `NAMA_*` layer skips it.
pub const FILE_ENV: &str = "NAMA_CONFIG";

/// Everything a Server needs to know before it starts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Config {
    /// The Server's display name, which clients show to tell Servers apart.
    pub name: String,
    /// Address the Server listens on.
    pub listen: SocketAddr,
    /// Directory where the Server keeps its database and other state.
    pub data_dir: PathBuf,
    /// Settings for each Plugin, by Plugin id, as `[plugins.<id>]` tables in the file. A Built-in
    /// Plugin with no table here is enabled.
    #[serde(default)]
    pub plugins: BTreeMap<String, PluginConfig>,
}

/// One Plugin's settings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginConfig {
    /// Whether the Server enables the Plugin at startup.
    #[serde(default = "enabled_by_default")]
    pub enabled: bool,
}

impl Default for PluginConfig {
    fn default() -> Self {
        Self {
            enabled: enabled_by_default(),
        }
    }
}

const fn enabled_by_default() -> bool {
    true
}

impl Default for Config {
    fn default() -> Self {
        Self {
            name: "Nama".to_owned(),
            listen: SocketAddr::from(([0, 0, 0, 0], 7980)),
            data_dir: PathBuf::from("data"),
            plugins: BTreeMap::new(),
        }
    }
}

impl Config {
    /// Loads the config. Later layers win: built-in defaults, then the TOML file, then `NAMA_*`
    /// environment variables.
    ///
    /// If `file` is given it must exist. If it isn't, [`DEFAULT_FILE`] in the working directory is
    /// read when present and skipped otherwise.
    ///
    /// # Errors
    ///
    /// If the named file is missing, or a layer has a value of the wrong type or syntax.
    pub fn load(file: Option<&Path>) -> Result<Self, Box<figment::Error>> {
        let file = file.or_else(|| Some(Path::new(DEFAULT_FILE)).filter(|f| f.is_file()));
        let mut figment = Figment::from(Serialized::defaults(Self::default()));
        if let Some(file) = file {
            figment = figment.merge(Toml::file_exact(file));
        }
        Ok(figment
            .merge(Env::prefixed(ENV_PREFIX).ignore(&["config"]))
            .extract()?)
    }
}
