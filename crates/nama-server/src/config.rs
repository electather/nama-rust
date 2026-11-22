//! Server configuration, layered from defaults, a TOML file and `NAMA_*` environment variables.

use std::{
    borrow::Cow,
    collections::BTreeMap,
    fmt,
    net::SocketAddr,
    path::{Path, PathBuf},
};

use figment::{
    Figment,
    providers::{Env, Format, Serialized, Toml},
    value::{Dict, Value},
};
use nama_plugin_api::settings::SettingValue;
use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::{Deserialize, Deserializer, Serialize};

pub use nama_forwarded::IpRange;

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
    /// Directory where the Server keeps its database and other state. A relative path counts from
    /// the folder the Server starts in.
    pub data_dir: PathBuf,
    /// The `[plugins.<id>]` tables in the file, imported into the database on the first start and
    /// ignored after.
    pub plugins: PluginTables,
    /// Reverse proxies whose `X-Forwarded-For` and `X-Forwarded-Proto` headers the Server believes.
    /// Empty by default, so the headers are ignored.
    // ADR 0018; kept out of the doc comment, which editors show users.
    pub trusted_proxies: Vec<IpRange>,
    /// The ffmpeg to run, such as a container's own. Unset, the default, looks for `ffmpeg` on
    /// `PATH`. A bare name, such as `jellyfin-ffmpeg`, is looked up on `PATH`, and a path with a
    /// folder in it, such as `bin/ffmpeg`, counts from the folder the Server starts in.
    pub ffmpeg: Option<PathBuf>,
    /// The ffprobe to run. Unset, the default, looks for `ffprobe` on `PATH`. A bare name is looked
    /// up on `PATH`, and a path with a folder in it, such as `./ffprobe`, counts from the folder
    /// the Server starts in.
    pub ffprobe: Option<PathBuf>,
    /// Whether the OS reports changes in Library folders so they're scanned soon after. On by
    /// default; off, folders are scanned every six hours and when asked.
    pub watch_folders: bool,
    /// Where the Server exports Diagnostics: the standard `OTEL_EXPORTER_OTLP_ENDPOINT`, which
    /// `nama-server` sets here once exporting has started, never a key in the file (ADR 0054).
    /// Unset, nothing is exported, and the web app records nothing.
    #[serde(skip)]
    #[schemars(skip)]
    pub otlp_endpoint: Option<String>,
    /// The config file the Server was started with, which its Backups hold: the one `--config` or
    /// `NAMA_CONFIG` names, or else `nama.toml` in the folder it starts in, whether or not it
    /// exists. `nama-server` sets it, never a key in the file.
    #[serde(skip)]
    #[schemars(skip)]
    pub file: PathBuf,
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
            otlp_endpoint: None,
            file: PathBuf::from(DEFAULT_FILE),
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
    /// If the named file is missing, a layer has a value of the wrong type or syntax, `data_dir` is
    /// empty, or the working folder can't be read to make a relative path absolute.
    pub fn load(file: Option<&Path>) -> Result<(Self, Vec<String>), Box<figment::Error>> {
        let file = file.or_else(|| Some(Path::new(DEFAULT_FILE)).filter(|f| f.is_file()));
        let mut figment = Figment::from(Serialized::defaults(Self::default()));
        if let Some(file) = file {
            figment = figment.merge(Toml::file_exact(file));
        }
        let figment = figment.merge(Env::prefixed(ENV_PREFIX).ignore(&["config"]));
        let NotingIgnored(mut config, ignored) = figment.extract()?;
        // The default segment folder is under `data_dir`, and ffmpeg runs inside it while also
        // handed paths that include it, so a relative one would resolve twice (#1120).
        config.data_dir = absolute("data_dir", &config.data_dir)?;
        for (key, program) in [
            ("ffmpeg", &mut config.ffmpeg),
            ("ffprobe", &mut config.ffprobe),
        ] {
            // A bare name is left as written, for `ffmpeg::finding` to look up on `PATH`.
            if let Some(path) = program.as_mut().filter(|p| p.components().count() > 1) {
                *path = absolute(key, path)?;
            }
        }
        // The file's own table, not the merged figment's metadata: a `NAMA_*` variable setting the
        // same key would hide the file's, and figment's paths split a quoted `"a.b"` key.
        let in_file: Dict = match file {
            Some(file) => Figment::from(Toml::file_exact(file)).extract()?,
            None => Dict::new(),
        };
        let unknown_keys = ignored
            .into_iter()
            .filter(|key| in_file.contains_key(key))
            .collect();
        Ok((config, unknown_keys))
    }
}

/// `path` made absolute from the working folder, lexically: the data directory may not exist yet,
/// and symlinks stay as the Admin wrote them. An empty `path` is an error naming `key`.
fn absolute(key: &str, path: &Path) -> Result<PathBuf, Box<figment::Error>> {
    std::path::absolute(path).map_err(|err| Box::new(format!("`{key}`: {err}").into()))
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
