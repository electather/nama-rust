//! Server configuration, layered from defaults, a TOML file and `NAMA_*` environment variables.

use std::{
    borrow::Cow,
    collections::BTreeMap,
    fmt,
    net::{IpAddr, SocketAddr},
    path::{Path, PathBuf},
    str::FromStr,
};

use figment::{
    Figment, Source,
    providers::{Env, Format, Serialized, Toml},
    value::{Dict, Value},
};
use nama_plugin_api::settings::SettingValue;
use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::{Deserialize, Deserializer, Serialize};

/// The config file read from the working directory when no file is named.
pub const DEFAULT_FILE: &str = "nama.toml";

/// Prefix of the environment variables that override the config file, e.g. `NAMA_DATA_DIR`.
pub const ENV_PREFIX: &str = "NAMA_";

/// Environment variable naming the config file. It picks the file rather than setting a key, so the
/// `NAMA_*` layer skips it.
pub const FILE_ENV: &str = "NAMA_CONFIG";

/// Everything a Server needs to know before it starts.
// Its doc comments are also `docs/nama.schema.json`'s descriptions, which editors show users.
// `Config::load` always supplies every key, so `default` changes no loading: it makes each key
// optional in the schema, with its default shown. The schema flags unknown keys; `Config::load`
// only reports them, so a stray key can't stop the Server (#842).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
#[schemars(deny_unknown_fields)]
pub struct Config {
    /// The Server's display name, which clients show to tell Servers apart.
    pub name: String,
    /// Address the Server listens on.
    pub listen: SocketAddr,
    /// Directory where the Server keeps its database and other state.
    pub data_dir: PathBuf,
    /// The `[plugins.<id>]` tables in the file, imported into the database on the first start and
    /// ignored after.
    pub plugins: PluginTables,
    /// Reverse proxies whose `X-Forwarded-For` and `X-Forwarded-Proto` headers the Server believes.
    /// Empty by default, so the headers are ignored.
    // ADR 0018; kept out of the doc comment, which editors show users.
    pub trusted_proxies: Vec<IpRange>,
    /// The ffmpeg to run, such as a container's own. Unset, the default, looks for it on `PATH`.
    pub ffmpeg: Option<PathBuf>,
    /// The ffprobe to run. Unset, the default, looks for it on `PATH`.
    pub ffprobe: Option<PathBuf>,
    /// Whether the OS reports changes in Library folders so they're scanned soon after. On by
    /// default; off, folders are scanned every six hours and when asked.
    pub watch_folders: bool,
}

/// The `[plugins.<id>]` tables in the file, by Plugin id, as written.
///
/// They are read as [`PluginConfig`]s only by [`PluginTables::parse`] when they are imported, so
/// once they are ignored a table that no longer parses can't stop the Server.
#[derive(Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PluginTables(BTreeMap<String, Dict>);

// Each Plugin declares its own settings at runtime, so the schema checks only `enabled`.
impl JsonSchema for PluginTables {
    fn inline_schema() -> bool {
        true
    }

    fn schema_name() -> Cow<'static, str> {
        "PluginTables".into()
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "type": "object",
            "additionalProperties": {
                "type": "object",
                "properties": { "enabled": { "type": "boolean" } },
            },
        })
    }
}

impl PluginTables {
    /// The Plugin ids the tables name.
    pub fn ids(&self) -> impl Iterator<Item = &str> {
        self.0.keys().map(String::as_str)
    }

    /// Each table read as its Plugin's config, by Plugin id.
    ///
    /// # Errors
    ///
    /// If a table has a value of the wrong type, such as a float setting.
    pub fn parse(&self) -> Result<BTreeMap<String, PluginConfig>, InvalidPluginTable> {
        self.0
            .iter()
            .map(|(id, table)| {
                Value::from(table.clone())
                    .deserialize()
                    .map(|config| (id.clone(), config))
                    .map_err(|source| InvalidPluginTable {
                        id: id.clone(),
                        source: Box::new(source),
                    })
            })
            .collect()
    }
}

// Values may be secrets, never logged (the crate's `AGENTS.md`), so only the Plugin ids are printed.
impl fmt::Debug for PluginTables {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_set().entries(self.0.keys()).finish()
    }
}

/// A `[plugins.<id>]` table that isn't a Plugin's config.
#[derive(Debug, thiserror::Error)]
#[error("the [plugins.{id}] table: {source}")]
pub struct InvalidPluginTable {
    id: String,
    source: Box<figment::Error>,
}

/// Whether one Plugin is enabled, and its settings: a `[plugins.<id>]` table in the file, and a row
/// in the database once imported (`plugin_settings`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginConfig {
    /// Whether the Server enables the Plugin. A table with no `enabled` line imports as enabled.
    #[serde(default = "enabled_by_default")]
    pub enabled: bool,
    /// The Plugin's settings: every other key in its table, such as `api_key = "..."`. They are
    /// checked against the schema the Plugin declares when it is enabled.
    #[serde(flatten)]
    pub settings: BTreeMap<String, SettingValue>,
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
            plugins: PluginTables::default(),
            trusted_proxies: Vec::new(),
            ffmpeg: None,
            ffprobe: None,
            watch_folders: true,
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
    /// Also returns the file's keys the Server doesn't know, which it ignores, such as a typo. A
    /// `NAMA_*` variable is never one, since unrelated programs use the prefix too.
    ///
    /// # Errors
    ///
    /// If the named file is missing, or a layer has a value of the wrong type or syntax.
    pub fn load(file: Option<&Path>) -> Result<(Self, Vec<String>), Box<figment::Error>> {
        let file = file.or_else(|| Some(Path::new(DEFAULT_FILE)).filter(|f| f.is_file()));
        let mut figment = Figment::from(Serialized::defaults(Self::default()));
        if let Some(file) = file {
            figment = figment.merge(Toml::file_exact(file));
        }
        let figment = figment.merge(Env::prefixed(ENV_PREFIX).ignore(&["config"]));
        let NotingIgnored(config, ignored) = figment.extract()?;
        // shortcut: a key also set by a `NAMA_*` variable, or a quoted key with a dot in it, isn't
        // reported; check the file's own table instead if one is missed in practice.
        let unknown_keys = ignored
            .into_iter()
            .filter(|key| {
                figment
                    .find_metadata(key)
                    .is_some_and(|metadata| matches!(metadata.source, Some(Source::File(_))))
            })
            .collect();
        Ok((config, unknown_keys))
    }
}

/// A [`Config`] and the paths of the keys deserializing it ignored.
// figment's own deserializer is private, so `serde_ignored` wraps it from inside `Deserialize`.
struct NotingIgnored(Config, Vec<String>);

impl<'de> Deserialize<'de> for NotingIgnored {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let mut ignored = Vec::new();
        let config =
            serde_ignored::deserialize(deserializer, |path| ignored.push(path.to_string()))?;
        Ok(Self(config, ignored))
    }
}

/// An IP address, or a CIDR range such as `10.0.0.0/8`, written as such in the config.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(try_from = "String", into = "String")]
#[schemars(inline)]
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
