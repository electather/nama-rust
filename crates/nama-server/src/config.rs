//! Server configuration, layered from defaults, a TOML file and `NAMA_*` environment variables.

use std::{
    collections::BTreeMap,
    fmt,
    net::{IpAddr, SocketAddr},
    path::{Path, PathBuf},
    str::FromStr,
};

use figment::{
    Figment,
    providers::{Env, Format, Serialized, Toml},
};
use nama_plugin_api::settings::SettingValue;
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
    /// Reverse proxies whose `X-Forwarded-For` and `X-Forwarded-Proto` headers the Server believes
    /// (ADR 0018). Empty by default, so the headers are ignored.
    #[serde(default)]
    pub trusted_proxies: Vec<IpRange>,
}

/// One Plugin's config: whether it is enabled, and its settings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginConfig {
    /// Whether the Server enables the Plugin at startup.
    #[serde(default = "enabled_by_default")]
    pub enabled: bool,
    /// The Plugin's settings: every other key in its table, such as `api_key = "..."`. They are
    /// checked against the schema the Plugin declares when it is enabled.
    #[serde(flatten)]
    pub settings: BTreeMap<String, SettingValue>,
}

impl Default for PluginConfig {
    fn default() -> Self {
        Self {
            enabled: enabled_by_default(),
            settings: BTreeMap::new(),
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
            trusted_proxies: Vec::new(),
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

/// An IP address, or a CIDR range such as `10.0.0.0/8`, written as such in the config.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct IpRange {
    network: IpAddr,
    prefix_len: u8,
}

impl IpRange {
    /// Whether `ip` is in the range. An IPv4 address mapped into IPv6, as a dual-stack socket
    /// reports an IPv4 peer, counts as that IPv4 address.
    #[must_use]
    pub fn contains(&self, ip: IpAddr) -> bool {
        let (network, width) = bits(self.network);
        let (ip, ip_width) = bits(ip.to_canonical());
        let host_bits = u32::from(width - self.prefix_len);
        width == ip_width && (network ^ ip).checked_shr(host_bits).unwrap_or(0) == 0
    }
}

/// `ip`'s bits, right-aligned, and how many there are.
fn bits(ip: IpAddr) -> (u128, u8) {
    match ip {
        IpAddr::V4(ip) => (u32::from(ip).into(), 32),
        IpAddr::V6(ip) => (ip.into(), 128),
    }
}

/// A trusted proxy that isn't an IP address or a CIDR range.
#[derive(Debug, thiserror::Error)]
#[error("`{0}` is not an IP address or a CIDR range")]
pub struct InvalidIpRange(String);

impl FromStr for IpRange {
    type Err = InvalidIpRange;

    fn from_str(range: &str) -> Result<Self, Self::Err> {
        let invalid = || InvalidIpRange(range.to_owned());
        let (network, prefix_len) = range
            .split_once('/')
            .map_or((range, None), |(network, len)| (network, Some(len)));
        let network = network.parse::<IpAddr>().ok().ok_or_else(invalid)?;
        let width = bits(network).1;
        let prefix_len = match prefix_len {
            None => width,
            Some(len) => len
                .parse()
                .ok()
                .filter(|&len| len <= width)
                .ok_or_else(invalid)?,
        };
        // `contains` compares an IPv4 address mapped into IPv6 as that IPv4 address, so a range
        // within the mapped block becomes the IPv4 range it maps, its prefix less the 96 bits
        // of the mapping.
        let canonical = network.to_canonical();
        Ok(match prefix_len.checked_sub(96) {
            Some(prefix_len) if canonical != network => Self {
                network: canonical,
                prefix_len,
            },
            _ => Self {
                network,
                prefix_len,
            },
        })
    }
}

impl TryFrom<String> for IpRange {
    type Error = InvalidIpRange;

    fn try_from(range: String) -> Result<Self, Self::Error> {
        range.parse()
    }
}

impl From<IpRange> for String {
    fn from(range: IpRange) -> Self {
        range.to_string()
    }
}

impl fmt::Display for IpRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.network, self.prefix_len)
    }
}
