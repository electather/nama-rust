//! Installed Plugins (ADR 0006): `.wasm` files bundled in the Server (ADR 0003) or that an admin
//! drops into `<data_dir>/plugins/`, run through Extism behind the same [`Plugin`] trait as a
//! Built-in Plugin, so the Kernel can't tell them apart.

use std::{
    collections::{BTreeMap, HashMap},
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
    BoxFuture, Observer, Plugin, PluginError, PluginId, Priority, Registrar,
    fulfiller::{Download, Fulfiller, Movies, Shows},
    notifier::{Notification, Notifier, Recipient},
    points::Events,
    settings::{
        Choice, Choices, SettingField, SettingKind, SettingValue, Settings, SettingsCheck,
        SettingsSchema,
    },
};
use nama_proto::{
    NAMA_API_VERSION,
    nama::{
        plugin::v1::{
            self as wire, Channel, CheckedSettings, Delivery, Fulfilled, Manifest, ObservePoint,
            ObservedEvents, Progress, ProgressAsked, ProvidePoint, setting_value,
        },
        v1::{Event, PluginKind, Request},
    },
};
use semver::{Version, VersionReq};
use thiserror::Error;
use tokio::task;
use tracing::{error, info, warn};

use super::{
    CheckTimedOut, Described, Registry, contain_sync, log_not_enabled, secret_values,
    without_secrets,
};
use crate::config::PluginConfig;

/// How long one call into an Installed Plugin may run, so one that never returns can't hold up
/// the start, its Observer's task or a settings check.
const CALL_TIMEOUT: Duration = Duration::from_secs(5);

/// How many calls in a row may crash before the Plugin is disabled, so one bad input doesn't
/// cost an admin a Plugin.
const CRASHES_BEFORE_DISABLING: u32 = 3;

/// The export that checks a Plugin's settings, if it has one.
const CHECK_SETTINGS: &str = "check_settings";

/// The Installed Plugins bundled in the Server, each by the name it is listed under if refused.
/// `build.rs` builds them.
const BUNDLED: &[(&str, &[u8])] = &[
    ("radarr.wasm", include_bytes!(env!("RADARR_WASM"))),
    ("sonarr.wasm", include_bytes!(env!("SONARR_WASM"))),
    ("telegram.wasm", include_bytes!(env!("TELEGRAM_WASM"))),
];

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
    #[error("its allowed host {0} has a port, and allowed hosts are matched without one")]
    HostWithPort(String),
    #[error("its id {0} is another Plugin's")]
    IdTaken(PluginId),
}

/// An Installed Plugin the Server read and checked, ready to enable.
#[derive(Clone)]
struct Installed {
    manifest: Arc<Manifest>,
    settings_schema: SettingsSchema,
    /// The `.wasm` file's contents. Extism fixes the hosts a Plugin may reach and the settings it
    /// reads when it compiles it, so each enabling compiles them again.
    wasm: Arc<[u8]>,
    cache_config: Arc<Path>,
    /// Whether it exports [`CHECK_SETTINGS`].
    checks_settings: bool,
}

/// An enabled Installed Plugin, compiled once with its allowed hosts and settings. Each call into
/// it gets a fresh instance, so one that traps or times out leaves nothing behind for the next.
#[derive(Clone)]
struct Running {
    manifest: Arc<Manifest>,
    compiled: Arc<CompiledPlugin>,
    /// The values of its secret settings, which the failures it logs mustn't show.
    secrets: Arc<[String]>,
    /// How many calls in a row crashed, counted apart for its Observer, its Fulfiller and its
    /// Notifier.
    crashes: Arc<AtomicU32>,
}

impl Running {
    /// Calls the Plugin's export `name` with `input`, a protobuf message, on a fresh instance.
    /// Blocks while the Plugin runs.
    fn call(&self, name: &str, input: Vec<u8>) -> Result<Vec<u8>, extism::Error> {
        extism::Plugin::new_from_compiled(&self.compiled)?.call(name, input)
    }

    /// [`Self::call`] on a blocking thread. A panic there unwinds here, so the registry disables
    /// the Plugin as it does one whose Rust code panics.
    async fn call_blocking(
        &self,
        name: &'static str,
        input: Vec<u8>,
    ) -> Result<Result<Vec<u8>, extism::Error>, task::JoinError> {
        let plugin = self.clone();
        match task::spawn_blocking(move || plugin.call(name, input)).await {
            Err(err) if err.is_panic() => panic::resume_unwind(err.into_panic()),
            called => called,
        }
    }

    /// [`Self::call_blocking`] for a Provider, whose caller gets why it failed, to log or show
    /// such as a Request's reason for being Failed: in the Plugin's own words, or that it crashed.
    /// Counts its crashes as [`Self::count_crash`] does.
    async fn provide(&self, name: &'static str, input: Vec<u8>) -> Result<Vec<u8>, PluginError> {
        let err = match self.call_blocking(name, input).await? {
            Ok(output) => {
                self.crashes.store(0, Ordering::Relaxed);
                return Ok(output);
            }
            Err(err) => err,
        };
        let cause = if is_timeout(&err) {
            format!("it didn't answer within {CALL_TIMEOUT:?}")
        } else {
            err.root_cause().to_string()
        };
        self.count_crash(&err, &without_secrets(cause.clone(), &self.secrets));
        Err(cause.into())
    }

    /// Logs why a call to its Observer failed, and counts it as [`Self::count_crash`] does.
    fn observer_failed(&self, err: &extism::Error) {
        let cause = without_secrets(err.root_cause().to_string(), &self.secrets);
        let failed = if is_crash(err) { "crashed" } else { "failed" };
        warn!(plugin = %self.manifest.id, "observer {failed}: {cause}");
        self.count_crash(err, &cause);
    }

    /// Counts the crashes in a row, `err` among them if it is one. On the
    /// [`CRASHES_BEFORE_DISABLING`]th, unwinds with why, `cause`, so the registry disables the
    /// Plugin as it does one whose Rust code panics.
    fn count_crash(&self, err: &extism::Error, cause: &str) {
        if !is_crash(err) {
            self.crashes.store(0, Ordering::Relaxed);
        } else if self.crashes.fetch_add(1, Ordering::Relaxed) + 1 == CRASHES_BEFORE_DISABLING {
            let why =
                format!("it crashed {CRASHES_BEFORE_DISABLING} times in a row, last with: {cause}");
            panic::resume_unwind(Box::new(why));
        }
    }
}

/// Whether a failed call trapped or timed out, rather than the Plugin returning an error or a
/// host function such as an HTTP request refusing it.
fn is_crash(err: &extism::Error) -> bool {
    // Any failed call but a timeout is wasmtime's own error, which holds a trap (under any panic's
    // message) only if the guest trapped. A host function's error unwinds the guest too, but holds
    // no trap.
    is_timeout(err)
        || err
            .downcast_ref::<wasmtime::Error>()
            .is_some_and(wasmtime::Error::is::<wasmtime::Trap>)
}

/// Whether a failed call ran past [`CALL_TIMEOUT`].
fn is_timeout(err: &extism::Error) -> bool {
    // Extism (1.41) gives a timeout as a bare "timeout".
    err.to_string() == "timeout"
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

    fn settings_check(&self) -> Option<&dyn SettingsCheck> {
        self.checks_settings.then_some(self)
    }

    // ponytail: compiles on the calling thread, an async one when an admin enables it; wasmtime's
    // cache makes that quick, so spawn_blocking only if a Plugin is slow to enable.
    fn register(&self, registrar: &mut Registrar) {
        let settings = registrar.settings();
        let hosts = allowed_hosts(&self.manifest, &self.settings_schema, settings);
        let config = extism_config(&self.settings_schema, settings);
        let compiled = match compile(&self.wasm, &self.cache_config, hosts, config) {
            Ok(compiled) => compiled,
            #[expect(
                clippy::panic,
                reason = "the registry contains the panic and refuses to enable the Plugin, saying why"
            )]
            Err(err) => panic!("could not compile it with its settings: {err:#}"),
        };
        let running = Running {
            manifest: Arc::clone(&self.manifest),
            compiled: Arc::new(compiled),
            secrets: secret_values(&self.settings_schema, settings).into(),
            crashes: Arc::default(),
        };
        // Each counting its own crashes, so its Observer working between them doesn't hide them.
        let with_own_crash_count = || {
            Arc::new(Running {
                crashes: Arc::default(),
                ..running.clone()
            })
        };
        let fulfiller = with_own_crash_count();
        // An Extension Point this Server doesn't know is one the Plugin can do without, since its
        // nama-api requirement admits this Server.
        let providers = &self.manifest.providers;
        if providers.contains(&ProvidePoint::PROVIDE_POINT_MOVIE_FULFILLER.into()) {
            registrar.provide::<dyn Fulfiller<Movies>>(
                Priority::DEFAULT,
                Arc::<Running>::clone(&fulfiller),
            );
        }
        if providers.contains(&ProvidePoint::PROVIDE_POINT_SHOW_FULFILLER.into()) {
            registrar.provide::<dyn Fulfiller<Shows>>(Priority::DEFAULT, fulfiller);
        }
        if providers.contains(&ProvidePoint::PROVIDE_POINT_NOTIFIER.into()) {
            registrar.provide::<dyn Notifier>(Priority::DEFAULT, with_own_crash_count());
        }
        if self
            .manifest
            .observers
            .contains(&ObservePoint::OBSERVE_POINT_EVENTS.into())
        {
            registrar.observe::<Events>(running);
        }
    }
}

// One impl for both kinds: the Plugin tells a Movie Request from a Season one by which it holds.
impl<Kind: 'static> Fulfiller<Kind> for Running {
    fn fulfill<'a>(&'a self, request: &'a Request) -> BoxFuture<'a, Result<String, PluginError>> {
        let input = request.encode_to_vec();
        Box::pin(async move {
            let output = self.provide("fulfill", input).await?;
            Ok(Fulfilled::decode_from_slice(&output)?.reference)
        })
    }

    fn progress<'a>(
        &'a self,
        references: &'a [String],
    ) -> BoxFuture<'a, Result<HashMap<String, Vec<Download>>, PluginError>> {
        let input = ProgressAsked {
            references: references.to_vec(),
            ..Default::default()
        }
        .encode_to_vec();
        Box::pin(async move {
            let output = self.provide("progress", input).await?;
            let downloads = Progress::decode_from_slice(&output)?.downloads;
            Ok(downloads
                .into_iter()
                .map(|(reference, downloads)| (reference, downloads.downloads))
                .collect())
        })
    }
}

impl Notifier for Running {
    fn deliver<'a>(
        &'a self,
        notification: &'a Notification,
        recipient: &'a Recipient,
    ) -> BoxFuture<'a, Result<(), PluginError>> {
        let recipient = match recipient {
            Recipient::Channel => wire::delivery::Recipient::from(Channel::default()),
        };
        let input = Delivery {
            notification: notification.clone().into(),
            recipient: Some(recipient),
            ..Default::default()
        }
        .encode_to_vec();
        Box::pin(async move { self.provide("deliver", input).await.map(drop) })
    }
}

impl Observer<Events> for Running {
    fn observe<'a>(&'a self, event: &'a Vec<Event>) -> BoxFuture<'a, ()> {
        let input = ObservedEvents {
            events: event.clone(),
            ..Default::default()
        }
        .encode_to_vec();
        Box::pin(async move {
            match self.call_blocking("observe_events", input).await {
                Ok(Ok(_)) => self.crashes.store(0, Ordering::Relaxed),
                Ok(Err(err)) => self.observer_failed(&err),
                Err(err) => warn!(plugin = %self.manifest.id, "observer failed: {err}"),
            }
        })
    }
}

impl SettingsCheck for Installed {
    fn check<'a>(&'a self, settings: &'a Settings) -> BoxFuture<'a, Result<Choices, PluginError>> {
        let hosts = allowed_hosts(&self.manifest, &self.settings_schema, settings);
        let config = extism_config(&self.settings_schema, settings);
        let wasm = Arc::clone(&self.wasm);
        let cache_config = Arc::clone(&self.cache_config);
        Box::pin(async move {
            // Compiled for this call alone, since the settings may be unsaved: the hosts of their
            // URL settings are allowed for it and gone once it returns.
            let called = task::spawn_blocking(move || {
                let compiled = compile(&wasm, &cache_config, hosts, config)?;
                extism::Plugin::new_from_compiled(&compiled)?
                    .call::<(), Vec<u8>>(CHECK_SETTINGS, ())
            })
            .await;
            let output = match called {
                Ok(Ok(output)) => output,
                Ok(Err(err)) if is_timeout(&err) => return Err(CheckTimedOut(CALL_TIMEOUT).into()),
                // So the registry logs why and fails the check in plain words, as it does when a
                // Built-in Plugin's check panics.
                Ok(Err(err)) if is_crash(&err) => {
                    panic::resume_unwind(Box::new(err.root_cause().to_string()))
                }
                Ok(Err(err)) => return Err(err.root_cause().to_string().into()),
                Err(err) if err.is_panic() => panic::resume_unwind(err.into_panic()),
                Err(err) => return Err(err.into()),
            };
            let checked = CheckedSettings::decode_from_slice(&output).map_err(|err| {
                format!(
                    "its {CHECK_SETTINGS}() returned something that isn't CheckedSettings: {err}"
                )
            })?;
            Ok(choices(checked))
        })
    }
}

/// The choices `checked` offers, as the Kernel takes them.
fn choices(checked: CheckedSettings) -> Choices {
    checked
        .choices
        .into_iter()
        .map(|(key, offered)| {
            let offered = offered.choices.into_iter().map(|choice| Choice {
                value: choice.value,
                label: choice.label,
            });
            (key, offered.collect())
        })
        .collect()
}

/// The bundled Installed Plugins and those in `<data_dir>/plugins/`, compiled but not yet listed.
pub(crate) struct Found(Vec<(PathBuf, Result<Installed, Refused>)>);

/// Reads the bundled Installed Plugins and those in `<data_dir>/plugins/`, for
/// [`Registry::list_installed`] to list.
///
/// Blocks while it compiles each Plugin. wasmtime caches what it compiles under
/// `<data_dir>/wasmtime/`, not in the home directory.
pub(crate) fn read(data_dir: &Path) -> Found {
    Found(read_plugins(data_dir))
}

impl Registry {
    /// Reads and lists the bundled Installed Plugins and those in `<data_dir>/plugins/` at start,
    /// as listing the Plugins does: logs each one's id and version, or why the Server refused it,
    /// and enables those `config` says are enabled. Installed Plugins start disabled, so one
    /// `config` doesn't name stays disabled.
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
                checks_settings: plugin.checks_settings,
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

/// The bundled Plugins, then each `.wasm` file in `<data_dir>/plugins/` in name order, each with
/// its Plugin or why the Server refused it. Bundled Plugins come first, so a file with one's id is
/// refused. Logs why it read no files, if it couldn't.
fn read_plugins(data_dir: &Path) -> Vec<(PathBuf, Result<Installed, Refused>)> {
    let dir = data_dir.join("plugins");
    let files = wasm_files(&dir).unwrap_or_else(|err| {
        error!(
            "could not list the Installed Plugins in {}: {err}",
            dir.display()
        );
        Vec::new()
    });
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
    let bundled = BUNDLED.iter().map(|&(name, wasm)| {
        let plugin = load(wasm.to_vec(), &cache_config);
        (Path::new("<bundled>").join(name), plugin)
    });
    let dropped_in = files.into_iter().map(|file| {
        let plugin = fs::read(&file)
            .map_err(Refused::Read)
            .and_then(|wasm| load(wasm, &cache_config));
        (file, plugin)
    });
    bundled.chain(dropped_in).collect()
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

/// Compiles the Plugin `wasm`, calls its `manifest()` and checks the Manifest it returns.
fn load(wasm: Vec<u8>, cache_config: &Path) -> Result<Installed, Refused> {
    // Extism would also take WASM's text format, and its parse errors span many lines.
    if !wasm.starts_with(WASM_MAGIC) {
        return Err(Refused::NotWasm);
    }
    let (bytes, checks_settings) = contain_sync(|| {
        let compiled =
            compile(&wasm, cache_config, Vec::new(), Vec::new()).map_err(Refused::Load)?;
        let mut plugin = extism::Plugin::new_from_compiled(&compiled).map_err(Refused::Manifest)?;
        let bytes = plugin
            .call::<(), Vec<u8>>("manifest", ())
            .map_err(Refused::Manifest)?;
        Ok((bytes, plugin.function_exists(CHECK_SETTINGS)))
    })
    .map_err(Refused::Panicked)??;
    let manifest = Manifest::decode_from_slice(&bytes).map_err(Refused::Decode)?;
    check_nama_api(&manifest.nama_api)?;
    check_allowed_hosts(&manifest.allowed_hosts)?;
    let settings_schema = settings_schema(&manifest)?;
    Ok(Installed {
        manifest: Arc::new(manifest),
        settings_schema,
        wasm: wasm.into(),
        cache_config: cache_config.into(),
        checks_settings,
    })
}

/// Compiles `wasm`, letting it send HTTP requests to `hosts`, read their responses' headers, and
/// read `config`.
fn compile(
    wasm: &[u8],
    cache_config: &Path,
    hosts: Vec<String>,
    config: Vec<(String, String)>,
) -> Result<CompiledPlugin, extism::Error> {
    let manifest = extism::Manifest::new([Wasm::data(wasm.to_vec())])
        .with_timeout(CALL_TIMEOUT)
        .with_allowed_hosts(hosts.into_iter())
        .with_config(config.into_iter());
    // Response headers include `Date`, the only clock a Plugin has: Sonarr's times a wait by it.
    CompiledPlugin::new(
        PluginBuilder::new(manifest)
            .with_cache_config(cache_config)
            .with_http_response_headers(true),
    )
}

/// The hosts a Plugin enabled with `settings` may send HTTP requests to: those its Manifest
/// names, and the host of each of its URL settings (ADR 0006).
fn allowed_hosts(manifest: &Manifest, schema: &SettingsSchema, settings: &Settings) -> Vec<String> {
    // Extism reads every allowed host as a glob, and a URL's host may hold `*` or, for IPv6, `[`:
    // escaped, it names only itself.
    let url_hosts = schema
        .fields
        .iter()
        .filter(|field| field.kind == SettingKind::Url)
        .filter_map(|field| Some(glob::Pattern::escape(settings.url(&field.key)?.host_str()?)));
    manifest
        .allowed_hosts
        .iter()
        .cloned()
        .chain(url_hosts)
        .collect()
}

/// `settings` as the Extism config a Plugin reads them from, each value as text: a bool as
/// `true` or `false`, a number in decimal.
fn extism_config(schema: &SettingsSchema, settings: &Settings) -> Vec<(String, String)> {
    schema
        .fields
        .iter()
        .filter_map(|field| {
            let text = match settings.get(&field.key)? {
                SettingValue::Bool(on) => on.to_string(),
                SettingValue::Integer(number) => number.to_string(),
                SettingValue::Text(text) => text.clone(),
            };
            Some((field.key.clone(), text))
        })
        .collect()
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

/// Checks that no host in a Manifest's `allowed_hosts` has a port, which Extism would never match.
fn check_allowed_hosts(hosts: &[String]) -> Result<(), Refused> {
    match hosts.iter().find(|host| host.contains(':')) {
        Some(host) => Err(Refused::HostWithPort(host.clone())),
        None => Ok(()),
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
                Some(wire::SettingKind::SETTING_KIND_URL) => SettingKind::Url,
                Some(wire::SettingKind::SETTING_KIND_CHOICE) => SettingKind::Choice,
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
    use nama_plugin_api::settings::{
        SettingField, SettingKind, SettingValue, Settings, SettingsSchema,
    };
    use nama_proto::nama::plugin::v1::{self as wire, Manifest, setting_value};

    use super::{allowed_hosts, check_allowed_hosts, extism_config, is_crash, settings_schema};

    // The errors as Extism 1.41 gives them for the test Plugin's Observer.
    #[test]
    fn a_trap_or_timeout_is_a_crash_and_a_returned_or_host_error_is_not() {
        // As Extism builds it: the panic's message, with the trap as context.
        let trap = wasmtime::Error::new(wasmtime::Trap::UnreachableCodeReached);
        let panicked = extism::Error::from(
            wasmtime::Error::msg("panicked at src/lib.rs:73:30: oops").context(trap),
        );
        let refused = extism::Error::msg("HTTP request to http://sonarr.lan/ is not allowed")
            .context("error while executing at wasm backtrace");

        assert!(is_crash(&panicked));
        assert!(is_crash(&extism::Error::msg("timeout")));
        assert!(!is_crash(&refused));
        assert!(!is_crash(&extism::Error::msg("the service is down")));
    }

    fn field(key: &str, kind: SettingKind) -> SettingField {
        SettingField {
            key: key.to_owned(),
            label: String::new(),
            description: String::new(),
            kind,
            required: false,
            default: None,
        }
    }

    fn text(text: &str) -> SettingValue {
        SettingValue::Text(text.to_owned())
    }

    #[test]
    fn a_plugin_may_reach_its_manifests_hosts_and_the_hosts_of_its_url_settings_only() {
        let manifest = Manifest {
            allowed_hosts: vec!["*.themoviedb.org".to_owned()],
            ..Default::default()
        };
        let schema = SettingsSchema {
            fields: vec![
                field("sonarr", SettingKind::Url),
                field("radarr", SettingKind::Url),
                field("note", SettingKind::Text),
            ],
        };
        let settings = Settings::new(
            [
                ("sonarr".to_owned(), text("http://sonarr.lan:8989/api")),
                ("note".to_owned(), text("http://not-a-url-setting.lan/")),
            ]
            .into(),
        );

        let hosts = allowed_hosts(&manifest, &schema, &settings);

        assert_eq!(hosts, ["*.themoviedb.org", "sonarr.lan"]);
    }

    #[test]
    fn a_url_settings_host_names_only_itself_not_a_glob() {
        let schema = SettingsSchema {
            fields: vec![
                field("any", SettingKind::Url),
                field("ipv6", SettingKind::Url),
            ],
        };
        let settings = Settings::new(
            [
                ("any".to_owned(), text("http://*/")),
                ("ipv6".to_owned(), text("http://[::1]:8989/")),
            ]
            .into(),
        );

        let hosts = allowed_hosts(&Manifest::default(), &schema, &settings);

        // Matched as Extism matches them.
        let reaches = |host: &str| {
            hosts
                .iter()
                .any(|allowed| glob::Pattern::new(allowed).is_ok_and(|glob| glob.matches(host)))
        };
        assert!(reaches("*"));
        assert!(reaches("[::1]"));
        assert!(!reaches("example.com"));
    }

    #[test]
    fn an_allowed_host_with_a_port_refuses_the_plugin() {
        let hosts = ["*.themoviedb.org".to_owned(), "sonarr.lan:8989".to_owned()];

        let refused = check_allowed_hosts(&hosts);

        assert_eq!(
            refused.err().map(|refused| refused.to_string()),
            Some(
                "its allowed host sonarr.lan:8989 has a port, and allowed hosts are matched without one"
                    .to_owned()
            )
        );
        let without_port = check_allowed_hosts(&["*.themoviedb.org".to_owned()]);
        assert_eq!(without_port.ok(), Some(()));
    }

    #[test]
    fn a_plugin_reads_each_setting_as_text() {
        let schema = SettingsSchema {
            fields: vec![
                field("on", SettingKind::Bool),
                field("count", SettingKind::Integer),
                field("name", SettingKind::Text),
                field("unset", SettingKind::Text),
            ],
        };
        let settings = Settings::new(
            [
                ("on".to_owned(), SettingValue::Bool(true)),
                ("count".to_owned(), SettingValue::Integer(-3)),
                ("name".to_owned(), text("Nama")),
            ]
            .into(),
        );

        let config = extism_config(&schema, &settings);

        let expected = [("on", "true"), ("count", "-3"), ("name", "Nama")]
            .map(|(key, value)| (key.to_owned(), value.to_owned()));
        assert_eq!(config, expected);
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
