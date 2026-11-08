//! Installed Plugins (ADR 0006): `.wasm` files an admin drops into `<data_dir>/plugins/`, run
//! through Extism behind the same [`Plugin`] trait as a Built-in Plugin, so the Kernel can't tell
//! them apart.

use std::{
    collections::BTreeMap,
    fs, io, panic,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicU32, Ordering},
    },
    time::Duration,
};

use buffa::Message;
use extism::{CompiledPlugin, PluginBuilder, Wasm};
use nama_plugin_api::{
    BoxFuture, Observer, Plugin, PluginId, Registrar,
    points::Events,
    settings::{SettingField, SettingKind, SettingValue, Settings, SettingsSchema},
};
use nama_proto::{
    NAMA_API_VERSION,
    nama::{
        plugin::v1::{self as wire, Manifest, ObservePoint, ObservedEvents, setting_value},
        v1::{Event, PluginKind},
    },
};
use semver::{Version, VersionReq};
use thiserror::Error;
use tokio::task;
use tracing::{error, info, warn};

use super::{Described, Registry, contain_sync, log_not_enabled, secret_values, without_secrets};
use crate::config::PluginConfig;

/// How long one call into an Installed Plugin may run, so one that never returns can't hold up
/// the start or its Observer's task.
const CALL_TIMEOUT: Duration = Duration::from_secs(5);

/// How many calls in a row may crash before the Plugin is disabled, so one bad input doesn't
/// cost an admin a Plugin.
const CRASHES_BEFORE_DISABLING: u32 = 3;

/// What every binary WASM module starts with.
const WASM_MAGIC: &[u8] = b"\0asm";

/// Why the Server refused an Installed Plugin.
#[derive(Debug, Error)]
enum Refused {
    #[error("could not read it: {0}")]
    Read(io::Error),
    #[error("it is not a WASM module")]
    NotWasm,
    #[error("the Server could not load it: {0:#}")]
    Load(extism::Error),
    // The root cause is the Plugin's own error or panic message, or the trap; extism logs the
    // wasm backtrace wrapped around it.
    #[error("its manifest() export failed: {}", .0.root_cause())]
    Manifest(extism::Error),
    #[error("the Server panicked while loading it: {0}")]
    Panicked(String),
    #[error("its manifest() returned something that isn't a Manifest: {0}")]
    Decode(buffa::DecodeError),
    #[error("its nama-api requirement {wants:?} isn't a semver requirement: {error}")]
    BadRequirement { wants: String, error: semver::Error },
    #[error("it wants nama-api {wants}, and this Server has {}", NAMA_API_VERSION)]
    OutOfRange { wants: String },
    #[error("its setting {key} is of a kind this Server doesn't know")]
    UnknownSettingKind { key: String },
    #[error("its setting {key}'s default isn't {kind:?}")]
    DefaultWrongKind { key: String, kind: SettingKind },
    #[error("its id {0} is another Plugin's")]
    IdTaken(PluginId),
}

/// An Installed Plugin the Server read, ready to be compiled with the settings it is enabled
/// with.
#[derive(Clone)]
struct Installed {
    manifest: Arc<Manifest>,
    settings_schema: SettingsSchema,
    /// Its module, with every call's timeout.
    module: Arc<extism::Manifest>,
    cache_config: Arc<Path>,
}

/// An enabled Installed Plugin, compiled with its settings. Each call gets a fresh instance,
/// since one that trapped or timed out may have stopped halfway through changing its own state.
struct Running {
    id: PluginId,
    compiled: Arc<CompiledPlugin>,
    /// The values of the secret settings it was enabled with, which its errors mustn't log.
    secrets: Vec<String>,
    /// How many calls in a row crashed.
    crashes: AtomicU32,
}

impl Running {
    /// Logs why a call failed. If it crashed [`CRASHES_BEFORE_DISABLING`] times in a row, unwinds
    /// with why, so the registry disables the Plugin as it does one whose Rust code panics.
    fn failed(&self, err: &extism::Error) {
        let cause = without_secrets(err.root_cause().to_string(), &self.secrets);
        if !is_crash(err) {
            self.crashes.store(0, Ordering::Relaxed);
            warn!(plugin = %self.id, "observer failed: {cause}");
            return;
        }
        warn!(plugin = %self.id, "observer crashed: {cause}");
        if self.crashes.fetch_add(1, Ordering::Relaxed) + 1 == CRASHES_BEFORE_DISABLING {
            let why =
                format!("it crashed {CRASHES_BEFORE_DISABLING} times in a row, last with: {cause}");
            panic::resume_unwind(Box::new(why));
        }
    }
}

/// Whether a failed call trapped or timed out, rather than the Plugin returning an error.
fn is_crash(err: &extism::Error) -> bool {
    // Extism (1.41) gives a returned error as a bare message, a timeout as a bare "timeout", and
    // a trap wrapped in its wasm backtrace, with any panic's message as the source.
    err.source().is_some() || err.to_string() == "timeout"
}

/// The Extism config value of a setting, which is how a Plugin reads its settings.
fn config_value(value: &SettingValue) -> String {
    match value {
        SettingValue::Bool(on) => on.to_string(),
        SettingValue::Integer(number) => number.to_string(),
        SettingValue::Text(text) => text.clone(),
    }
}

fn compile(module: extism::Manifest, cache_config: &Path) -> Result<CompiledPlugin, extism::Error> {
    CompiledPlugin::new(PluginBuilder::new(module).with_cache_config(cache_config))
}

impl Plugin for Installed {
    fn id(&self) -> PluginId {
        PluginId::from(self.manifest.id.clone())
    }

    fn version(&self) -> &str {
        &self.manifest.version
    }

    fn settings_schema(&self) -> SettingsSchema {
        self.settings_schema.clone()
    }

    fn register(&self, registrar: &mut Registrar) {
        let settings = registrar.settings();
        let config = self.settings_schema.fields.iter().filter_map(|field| {
            let value = settings.get(&field.key)?;
            Some((field.key.clone(), config_value(value)))
        });
        let module = (*self.module).clone().with_config(config);
        // It compiled when the Server read it, and wasmtime's cache has it, so this failing is the
        // Server's fault; enabling reports a panic as a failure.
        // ponytail: compiles on the enabling thread, about 2 ms from the cache (spike #11); move
        // it off the runtime if Plugins get big.
        #[expect(clippy::panic, reason = "`register` can't return an error")]
        let compiled = compile(module, &self.cache_config)
            .unwrap_or_else(|err| panic!("could not compile it with its settings: {err:#}"));
        let running = Running {
            id: self.id(),
            compiled: Arc::new(compiled),
            secrets: secret_values(&self.settings_schema, settings),
            crashes: AtomicU32::new(0),
        };
        // An Extension Point this Server doesn't know is one the Plugin can do without, since its
        // nama-api requirement admits this Server.
        if self
            .manifest
            .observers
            .contains(&ObservePoint::OBSERVE_POINT_EVENTS.into())
        {
            registrar.observe::<Events>(running);
        }
    }
}

impl Observer<Events> for Running {
    fn observe<'a>(&'a self, event: &'a Vec<Event>) -> BoxFuture<'a, ()> {
        let input = ObservedEvents {
            events: event.clone(),
            ..Default::default()
        }
        .encode_to_vec();
        let compiled = Arc::clone(&self.compiled);
        Box::pin(async move {
            let called = task::spawn_blocking(move || {
                extism::Plugin::new_from_compiled(&compiled)?
                    .call::<_, Vec<u8>>("observe_events", input)
            })
            .await;
            match called {
                Ok(Ok(_)) => self.crashes.store(0, Ordering::Relaxed),
                Ok(Err(err)) => self.failed(&err),
                // So the registry disables the Plugin, as it does one whose Rust code panics.
                Err(err) if err.is_panic() => panic::resume_unwind(err.into_panic()),
                Err(err) => warn!(plugin = %self.id, "observer failed: {err}"),
            }
        })
    }
}

/// The Installed Plugins in `<data_dir>/plugins/`, compiled but not yet listed.
pub(crate) struct Found(Vec<(PathBuf, Result<Installed, Refused>)>);

/// Reads the Installed Plugins in `<data_dir>/plugins/`, for [`Registry::list_installed`] to list.
///
/// Blocks while it compiles each Plugin. wasmtime caches what it compiles under
/// `<data_dir>/wasmtime/`, not in the home directory.
pub(crate) fn read(data_dir: &Path) -> Found {
    Found(read_plugins_dir(data_dir))
}

impl Registry {
    /// Reads and lists the Installed Plugins in `<data_dir>/plugins/` at start, as listing the
    /// Plugins does: logs each one's id and version, or why the Server refused it, and enables
    /// those `config` says are enabled. Installed Plugins start disabled, so one `config` doesn't
    /// name stays disabled.
    pub fn load_installed(&self, data_dir: &Path, config: &BTreeMap<String, PluginConfig>) {
        let found = read(data_dir);
        for (file, plugin) in &found.0 {
            match plugin {
                Ok(plugin) => info!(
                    file = %file.display(),
                    id = %plugin.manifest.id,
                    version = %plugin.manifest.version,
                    "found Installed Plugin"
                ),
                Err(refused) => {
                    warn!(file = %file.display(), "refused Installed Plugin: {refused}");
                }
            }
        }
        self.list_installed(found);
        for (id, stored) in config {
            let id = PluginId::from(id.clone());
            let Some(kind) = self.read(|state| state.known.get(&id).map(|known| known.kind)) else {
                warn!(plugin = %id, "settings name a plugin this Server doesn't have");
                continue;
            };
            if kind == PluginKind::PLUGIN_KIND_INSTALLED
                && stored.enabled
                && let Err(err) = self.enable_by_id(&id, Settings::new(stored.settings.clone()))
            {
                log_not_enabled(&id, &err);
            }
        }
    }

    /// Lists the Installed Plugins `found` beside the Built-in ones, in place of those listed
    /// before. An enabled Plugin stays listed as it was enabled, even if its file is gone or
    /// changed, until it is disabled. Enables none.
    pub(crate) fn list_installed(&self, found: Found) {
        let mut taken: Vec<PluginId> = self.read(|state| {
            let builtin = |(id, known): (&PluginId, &Described)| {
                (known.kind == PluginKind::PLUGIN_KIND_BUILT_IN).then(|| id.clone())
            };
            state.known.iter().filter_map(builtin).collect()
        });
        let mut described = Vec::new();
        for (file, plugin) in found.0 {
            let (id, plugin) = describe(&file, plugin, &taken);
            if plugin.make.is_ok() {
                taken.push(id.clone());
            }
            described.push((id, plugin));
        }
        self.write(|state| {
            let enabled = &state.plugins;
            state.known.retain(|id, known| {
                known.kind == PluginKind::PLUGIN_KIND_BUILT_IN || enabled.contains_key(id)
            });
            for (id, plugin) in described {
                state.known.entry(id).or_insert(plugin);
            }
        });
    }
}

/// The Installed Plugin in `file` as the registry lists it: by its id, or by the file's name if
/// the Server refused it, since it may not know the id. One whose id is `taken` is refused.
fn describe(
    file: &Path,
    plugin: Result<Installed, Refused>,
    taken: &[PluginId],
) -> (PluginId, Described) {
    let plugin = plugin.and_then(|plugin| match plugin.id() {
        id if taken.contains(&id) => Err(Refused::IdTaken(id)),
        _ => Ok(plugin),
    });
    match plugin {
        Ok(plugin) => {
            let id = plugin.id();
            let described = Described {
                kind: PluginKind::PLUGIN_KIND_INSTALLED,
                version: plugin.manifest.version.clone(),
                settings_schema: plugin.settings_schema.clone(),
                checks_settings: false,
                make: Ok(Arc::new(move || Arc::new(plugin.clone()))),
            };
            (id, described)
        }
        Err(refused) => {
            let name = file.file_name().unwrap_or_default().to_string_lossy();
            let described = Described {
                kind: PluginKind::PLUGIN_KIND_INSTALLED,
                version: String::new(),
                settings_schema: SettingsSchema::default(),
                checks_settings: false,
                make: Err(refused.to_string()),
            };
            (PluginId::from(name.into_owned()), described)
        }
    }
}

/// Each `.wasm` file in `<data_dir>/plugins/`, in name order, with its Plugin or why the Server
/// refused it. Logs why it read none, if it couldn't.
fn read_plugins_dir(data_dir: &Path) -> Vec<(PathBuf, Result<Installed, Refused>)> {
    let dir = data_dir.join("plugins");
    let files = match wasm_files(&dir) {
        Ok(files) if files.is_empty() => return Vec::new(),
        Ok(files) => files,
        Err(err) => {
            error!(
                "could not list the Installed Plugins in {}: {err}",
                dir.display()
            );
            return Vec::new();
        }
    };
    let cache_config = match write_cache_config(data_dir) {
        Ok(cache_config) => cache_config,
        Err(err) => {
            error!(
                "could not set up wasmtime's compile cache in {}: {err}",
                data_dir.display()
            );
            return Vec::new();
        }
    };
    // ponytail: one Plugin at a time, each up to CALL_TIMEOUT, at start and on every listing;
    // compile them in parallel, or only changed files, if many Plugins make that slow.
    files
        .into_iter()
        .map(|file| {
            let plugin = load(&file, &cache_config);
            (file, plugin)
        })
        .collect()
}

/// The `.wasm` files in `dir`, sorted by name. A missing `dir` holds none.
fn wasm_files(dir: &Path) -> io::Result<Vec<PathBuf>> {
    let entries = match fs::read_dir(dir) {
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        entries => entries?,
    };
    let mut files = Vec::new();
    for entry in entries {
        let path = entry?.path();
        if path
            .extension()
            .is_some_and(|extension| extension == "wasm")
        {
            files.push(path);
        }
    }
    files.sort();
    Ok(files)
}

/// Writes the wasmtime cache config that Extism reads, pointing the cache at
/// `<data_dir>/wasmtime/cache/`, and returns its path. Extism takes the cache's location only
/// from such a file.
fn write_cache_config(data_dir: &Path) -> io::Result<PathBuf> {
    // wasmtime requires an absolute cache directory.
    let dir = std::path::absolute(data_dir)?.join("wasmtime");
    fs::create_dir_all(&dir)?;
    // A JSON string is also a valid TOML basic string, escapes included.
    let cache_dir = serde_json::to_string(&dir.join("cache"))?;
    let file = dir.join("cache.toml");
    fs::write(&file, format!("[cache]\ndirectory = {cache_dir}\n"))?;
    Ok(file)
}

/// Compiles the Plugin in `file`, calls its `manifest()` and checks the Manifest it returns.
fn load(file: &Path, cache_config: &Path) -> Result<Installed, Refused> {
    let wasm = fs::read(file).map_err(Refused::Read)?;
    // Extism would also take WASM's text format, and its parse errors span many lines.
    if !wasm.starts_with(WASM_MAGIC) {
        return Err(Refused::NotWasm);
    }
    let module = extism::Manifest::new([Wasm::data(wasm)]).with_timeout(CALL_TIMEOUT);
    let bytes = contain_sync(|| {
        let compiled = compile(module.clone(), cache_config).map_err(Refused::Load)?;
        extism::Plugin::new_from_compiled(&compiled)
            .and_then(|mut plugin| plugin.call::<(), Vec<u8>>("manifest", ()))
            .map_err(Refused::Manifest)
    })
    .map_err(Refused::Panicked)??;
    let manifest = Manifest::decode_from_slice(&bytes).map_err(Refused::Decode)?;
    check_nama_api(&manifest.nama_api)?;
    let settings_schema = settings_schema(&manifest)?;
    Ok(Installed {
        manifest: Arc::new(manifest),
        settings_schema,
        module: Arc::new(module),
        cache_config: cache_config.into(),
    })
}

/// Checks that this Server's Nama API version meets `wants`, a Plugin's semver requirement.
fn check_nama_api(wants: &str) -> Result<(), Refused> {
    let requirement = VersionReq::parse(wants).map_err(|error| Refused::BadRequirement {
        wants: wants.to_owned(),
        error,
    })?;
    // Cargo only accepts semver package versions, so this parses.
    if Version::parse(NAMA_API_VERSION).is_ok_and(|server| requirement.matches(&server)) {
        Ok(())
    } else {
        Err(Refused::OutOfRange {
            wants: wants.to_owned(),
        })
    }
}

fn settings_schema(manifest: &Manifest) -> Result<SettingsSchema, Refused> {
    let fields = manifest
        .settings
        .iter()
        .map(|field| {
            let kind = match field.kind.as_known() {
                Some(wire::SettingKind::SETTING_KIND_TEXT) => SettingKind::Text,
                Some(wire::SettingKind::SETTING_KIND_SECRET) => SettingKind::Secret,
                Some(wire::SettingKind::SETTING_KIND_BOOL) => SettingKind::Bool,
                Some(wire::SettingKind::SETTING_KIND_INTEGER) => SettingKind::Integer,
                Some(wire::SettingKind::SETTING_KIND_UNSPECIFIED) | None => {
                    return Err(Refused::UnknownSettingKind {
                        key: field.key.clone(),
                    });
                }
            };
            let default = field
                .default_value
                .as_option()
                .and_then(|default| default.value.as_ref())
                .map(|value| match value {
                    setting_value::Value::Bool(on) => SettingValue::Bool(*on),
                    setting_value::Value::Integer(number) => SettingValue::Integer(*number),
                    setting_value::Value::Text(text) => SettingValue::Text(text.clone()),
                });
            if default.as_ref().is_some_and(|default| !default.is(kind)) {
                return Err(Refused::DefaultWrongKind {
                    key: field.key.clone(),
                    kind,
                });
            }
            Ok(SettingField {
                key: field.key.clone(),
                label: field.label.clone(),
                description: field.description.clone(),
                kind,
                required: field.required,
                default,
            })
        })
        .collect::<Result<_, _>>()?;
    Ok(SettingsSchema { fields })
}

#[cfg(test)]
mod tests {
    use nama_plugin_api::settings::{SettingField, SettingKind, SettingValue};
    use nama_proto::nama::plugin::v1::{self as wire, Manifest, setting_value};

    use super::{is_crash, settings_schema};

    // The errors as Extism 1.41 gives them for the test Plugin's Observer.
    #[test]
    fn a_trap_or_timeout_is_a_crash_and_a_returned_error_is_not() {
        let panicked = extism::Error::msg("panicked at src/lib.rs:73:30: oops")
            .context("error while executing at wasm backtrace");

        assert!(is_crash(&panicked));
        assert!(is_crash(&extism::Error::msg("timeout")));
        assert!(!is_crash(&extism::Error::msg("the service is down")));
    }

    fn manifest(kind: wire::SettingKind) -> Manifest {
        let field = wire::SettingField {
            key: "api_key".to_owned(),
            label: "API key".to_owned(),
            description: "The key.".to_owned(),
            kind: kind.into(),
            required: true,
            default_value: wire::SettingValue {
                value: Some(setting_value::Value::Text("none".to_owned())),
                ..Default::default()
            }
            .into(),
            ..Default::default()
        };
        Manifest {
            settings: vec![field],
            ..Default::default()
        }
    }

    #[test]
    fn a_manifests_settings_become_its_settings_schema() {
        let schema = settings_schema(&manifest(wire::SettingKind::SETTING_KIND_SECRET)).ok();

        let expected = SettingField {
            key: "api_key".to_owned(),
            label: "API key".to_owned(),
            description: "The key.".to_owned(),
            kind: SettingKind::Secret,
            required: true,
            default: Some(SettingValue::Text("none".to_owned())),
        };
        assert_eq!(schema.map(|schema| schema.fields), Some(vec![expected]));
    }

    #[test]
    fn a_default_of_another_kind_refuses_the_plugin() {
        let refused = settings_schema(&manifest(wire::SettingKind::SETTING_KIND_BOOL));

        assert_eq!(
            refused.err().map(|refused| refused.to_string()),
            Some("its setting api_key's default isn't Bool".to_owned())
        );
    }

    #[test]
    fn a_setting_of_no_known_kind_refuses_the_plugin() {
        let refused = settings_schema(&manifest(wire::SettingKind::SETTING_KIND_UNSPECIFIED));

        assert_eq!(
            refused.err().map(|refused| refused.to_string()),
            Some("its setting api_key is of a kind this Server doesn't know".to_owned())
        );
    }
}
