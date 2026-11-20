//! Reading, compiling and checking an Installed Plugin's Package.

use std::{
    fs, io, mem,
    ops::Deref,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use buffa::{
    DecodeError, Message,
    encoding::{self, Tag, WireType},
    types,
};
use extism::{CompiledPlugin, CurrentPlugin, PluginBuilder, UserData, Val, ValType, Wasm};
use nama_plugin_api::{
    Manifest, PluginId,
    automation::Kernel,
    metadata::{ExternalId, InvalidId, SourceManifest},
    settings::{SettingField, SettingKind, SettingValue, Settings, SettingsSchema},
};
use nama_plugin_proto::{CONTRACT_VERSION, nama::plugin::v1 as wire, services};
use semver::{Version, VersionReq};
use tokio::runtime::Handle;
use tracing::error;

use super::{
    Installed, Point, Refused,
    automation::{KERNEL_IMPORTS, Reach, import},
    bundled::BUNDLED,
    calls::Calls,
    check_exports, contract,
    plugin_log::{self, PluginLog},
    points,
    services::{CHECK_SETTINGS, TITLE_SEARCH, known_exports},
};
use crate::plugins::{contain_sync, kept_logo, registry::personal_settings_refusal};

// Changing it changes what the Kernel gives a Plugin: see `crates/nama-plugin-proto/AGENTS.md`.
/// How long one call into an Installed Plugin may run, so one that never returns can't hold up
/// the start, its Observer's task or a settings check. For an enabled Plugin it counts only the
/// Plugin's own time, not the Kernel's answering its `nama` imports.
pub(super) const CALL_TIMEOUT: Duration = Duration::from_secs(5);

// Changing it changes what the Kernel gives a Plugin: see `crates/nama-plugin-proto/AGENTS.md`.
/// How long a tick may run, of the Plugin's own time: longer than [`CALL_TIMEOUT`], since a tick
/// may page through every title of a large Library and each title's Watch States.
pub(super) const TICK_TIMEOUT: Duration = Duration::from_secs(60);

// Changing it changes what the Kernel gives a Plugin: see `crates/nama-plugin-proto/AGENTS.md`.
/// How long a blob store call may run, of the Plugin's own time: longer than [`CALL_TIMEOUT`],
/// since one call carries a whole object, up to `BLOB_MAX_BYTES`, to or from an outside store.
pub(super) const BLOB_TIMEOUT: Duration = Duration::from_secs(60);

// Changing it changes what the Kernel gives a Plugin: see `crates/nama-plugin-proto/AGENTS.md`.
/// How much memory one call into an Installed Plugin may use, in 64 KiB pages: the guest's own and
/// what Extism keeps for it, so one Plugin can't take the Server down.
// ponytail: one cap for all; let a Manifest ask for more once a Plugin needs it.
pub(super) const MEMORY_MAX_PAGES: u32 = 2048;

/// The shortest a Plugin's `tick_every_seconds` may be, so no Plugin keeps the Kernel busy.
pub(super) const MIN_TICK_EVERY: Duration = Duration::from_secs(60);

/// What every binary WASM module starts with.
const WASM_MAGIC: &[u8] = b"\0asm";

/// The bundled Plugins, then each `.wasm` file in `<data_dir>/plugins/` in name order, each with
/// its Plugin or why the Server refused it. Bundled Plugins come first, so a file with one's id is
/// refused. Logs why it read no files, if it couldn't.
pub(super) fn read_plugins(data_dir: &Path) -> Vec<(PathBuf, Result<Installed, Refused>)> {
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
pub(super) fn write_cache_config(data_dir: &Path) -> io::Result<PathBuf> {
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
pub(super) fn load(wasm: Vec<u8>, cache_config: &Path) -> Result<Installed, Refused> {
    // Extism would also take WASM's text format, and its parse errors span many lines.
    if !wasm.starts_with(WASM_MAGIC) {
        return Err(Refused::NotWasm);
    }
    let (bytes, exported): (_, Vec<&'static str>) = contain_sync(|| {
        let compiled =
            compile(&wasm, cache_config, Vec::new(), Vec::new()).map_err(Refused::Load)?;
        let mut plugin =
            extism::Plugin::new_from_compiled(&compiled.plugin).map_err(Refused::Manifest)?;
        let bytes = plugin
            .call::<(), Vec<u8>>("manifest", ())
            .map_err(Refused::Manifest)?;
        let exported = known_exports()
            .filter(|export| plugin.function_exists(export))
            .collect();
        Ok((bytes, exported))
    })
    .map_err(Refused::Panicked)??;
    let mut package = decode(&bytes)?;
    Ok(Installed {
        manifest: judge(&mut package, &exported)?,
        package: Arc::new(package),
        wasm: wasm.into(),
        cache_config: cache_config.into(),
        checks_settings: exported.contains(&CHECK_SETTINGS),
        exports: Arc::new(exported.into_iter().collect()),
    })
}

/// `bytes` as a Manifest, once its Plugin contract requirement matches this Server's: a package
/// built for another Plugin contract may encode the rest differently, so it is refused for that
/// rather than found undecodable.
pub(super) fn decode(bytes: &[u8]) -> Result<wire::Manifest, Refused> {
    check_contract(&contract_requirement(bytes).map_err(Refused::Decode)?)?;
    wire::Manifest::decode_from_slice(bytes).map_err(Refused::Decode)
}

/// The `contract` of the encoded Manifest `bytes`, its field 3, read alone. `manifest.proto`
/// keeps that number in every Plugin contract version.
fn contract_requirement(mut bytes: &[u8]) -> Result<String, DecodeError> {
    let mut wants = String::new();
    while !bytes.is_empty() {
        let tag = Tag::decode(&mut bytes)?;
        if tag.field_number() == 3 && tag.wire_type() == WireType::LengthDelimited {
            wants = types::decode_string(&mut bytes)?;
        } else {
            encoding::skip_field(tag, &mut bytes)?;
        }
    }
    Ok(wants)
}

/// What the Server makes of `package`, the Manifest of a Package with the exports `exported`: the
/// Manifest the Kernel reads, or why it refuses the Plugin. Runs no Plugin code, so a test needs
/// no Package.
pub(super) fn judge(package: &mut wire::Manifest, exported: &[&str]) -> Result<Manifest, Refused> {
    check_allowed_hosts(&package.allowed_hosts)?;
    check_namespaces(package)?;
    let points = points(package)?;
    // A Manifest from before ADR 0043 named its points in fields since dropped, so it reads as
    // naming none and would load serving nothing.
    if points.is_empty() {
        return Err(Refused::NoPoints);
    }
    check_title_namespace(package, &points)?;
    check_tick_every(package, &points)?;
    check_exports(&points, |export| exported.contains(&export))?;
    manifest(package, &points, exported)
}

/// What `package`, naming `points` and with the exports `exported`, says about its Plugin, as the
/// Kernel reads it. Its logo is taken out, so the package kept with the Plugin never holds one
/// the Server dropped.
fn manifest(
    package: &mut wire::Manifest,
    points: &[&Point],
    exported: &[&str],
) -> Result<Manifest, Refused> {
    let settings = settings_schema(package)?;
    let personal_settings = schema(&package.personal_settings)?;
    let permissions = package
        .permissions
        .iter()
        .map(|permission| {
            permission
                .as_known()
                .filter(|known| *known != wire::Permission::PERMISSION_UNSPECIFIED)
                .ok_or_else(|| Refused::UnknownPermission(permission.to_string()))
        })
        .collect::<Result<_, _>>()?;
    let id = PluginId::from(package.id.clone());
    let logo_svg = kept_logo(&id, &mem::take(&mut package.logo_svg));
    let manifest = Manifest {
        id,
        name: package.name.clone(),
        description: package.description.clone(),
        version: package.version.clone(),
        settings,
        personal_settings,
        extension_points: points.iter().map(|point| (point.key)()).collect(),
        logo_svg,
        permissions,
        tick_every: names(points, services::AUTOMATION)
            .then(|| Duration::from_secs(package.tick_every_seconds.into())),
        title_source: names(points, services::TITLE_SOURCE).then(|| SourceManifest {
            namespace: package.title_namespace.clone(),
            looks_up: package.title_lookup_namespaces.clone(),
            searches: exported.contains(&TITLE_SEARCH.export),
        }),
    };
    match personal_settings_refusal(&manifest) {
        Some(reason) => Err(Refused::PersonalSettings(reason)),
        None => Ok(manifest),
    }
}

/// A compiled Installed Plugin, with the calls running on it.
pub(super) struct Compiled {
    pub(super) plugin: CompiledPlugin,
    /// The calls running on [`Self::plugin`], which its `nama` imports pause the clock of.
    pub(super) calls: Calls,
}

impl Deref for Compiled {
    type Target = CompiledPlugin;

    fn deref(&self) -> &CompiledPlugin {
        &self.plugin
    }
}

/// [`compile_with_kernel`] for no enablement, so the Kernel's functions answer `refused`. Its calls
/// keep Extism's own [`CALL_TIMEOUT`], which `manifest()` and `check_settings()` run under:
/// with no Kernel to wait for, all their time is the Plugin's.
pub(super) fn compile(
    wasm: &[u8],
    cache_config: &Path,
    hosts: Vec<String>,
    config: Vec<(String, String)>,
) -> Result<Compiled, extism::Error> {
    let calls = Calls::default();
    let manifest = manifest_for(wasm, hosts, config).with_timeout(CALL_TIMEOUT);
    let plugin = build(manifest, cache_config, None, None, calls.clone())?;
    Ok(Compiled { plugin, calls })
}

// Changing it changes what the Kernel gives a Plugin: see `crates/nama-plugin-proto/AGENTS.md`.
/// Compiles `wasm`, letting it send HTTP requests to `hosts`, read `config`, call [`now`], write
/// to `log` and call the Kernel's functions, which reach `kernel` on the runtime this is called
/// on. Its calls run in `calls`, which limits each to its method's time, so Extism's own timeout
/// is left off.
#[expect(
    clippy::too_many_arguments,
    reason = "each is a separate input of the compile, and every caller names them all"
)]
pub(super) fn compile_with_kernel(
    wasm: &[u8],
    cache_config: &Path,
    hosts: Vec<String>,
    config: Vec<(String, String)>,
    kernel: Option<Arc<dyn Kernel>>,
    log: Arc<PluginLog>,
    calls: Calls,
) -> Result<Compiled, extism::Error> {
    let manifest = manifest_for(wasm, hosts, config);
    let plugin = build(manifest, cache_config, kernel, Some(log), calls.clone())?;
    Ok(Compiled { plugin, calls })
}

fn manifest_for(
    wasm: &[u8],
    hosts: Vec<String>,
    config: Vec<(String, String)>,
) -> extism::Manifest {
    extism::Manifest::new([Wasm::data(wasm.to_vec())])
        .with_memory_max(MEMORY_MAX_PAGES)
        .with_allowed_hosts(hosts.into_iter())
        .with_config(config.into_iter())
}

/// Compiles `manifest`'s Plugin with every `nama` import. With no `log`, for no enablement, its
/// lines are dropped.
fn build(
    manifest: extism::Manifest,
    cache_config: &Path,
    kernel: Option<Arc<dyn Kernel>>,
    log: Option<Arc<PluginLog>>,
    calls: Calls,
) -> Result<CompiledPlugin, extism::Error> {
    let reach = UserData::new(Reach {
        kernel,
        runtime: Handle::try_current().ok(),
        calls,
    });
    // Every import is defined whether or not the Plugin may call it, so a Plugin importing them
    // still instantiates to read its Manifest or check its settings.
    let builder = KERNEL_IMPORTS.iter().fold(
        PluginBuilder::new(manifest)
            .with_cache_config(cache_config)
            .with_function_in_namespace("nama", "now", [], [ValType::I64], UserData::new(()), now)
            .with_function_in_namespace(
                "nama",
                "log",
                [ValType::I32, ValType::I64],
                [],
                UserData::new(log),
                plugin_log::import,
            ),
        |builder, &function| {
            builder.with_function_in_namespace(
                "nama",
                function,
                [ValType::I64],
                [ValType::I64],
                reach.clone(),
                import(function),
            )
        },
    );
    CompiledPlugin::new(builder)
}

/// `nama.now`, a Plugin's clock, since `SystemTime::now()` panics on wasm32-unknown-unknown:
/// writes the milliseconds since the Unix epoch by the Server's clock.
#[expect(clippy::unnecessary_wraps, reason = "Extism's signature")]
fn now(
    _: &mut CurrentPlugin,
    _: &[Val],
    out: &mut [Val],
    _: UserData<()>,
) -> Result<(), extism::Error> {
    out.fill(Val::I64(crate::db::now_ms()));
    Ok(())
}

/// The hosts a Plugin enabled with `settings` may send HTTP requests to: those its Manifest
/// names, and the host of each of its URL settings (ADR 0006).
pub(super) fn allowed_hosts(
    package: &wire::Manifest,
    schema: &SettingsSchema,
    settings: &Settings,
) -> Vec<String> {
    // Extism reads every allowed host as a glob, and a URL's host may hold `*` or, for IPv6, `[`:
    // escaped, it names only itself.
    let url_hosts = schema
        .fields
        .iter()
        .filter(|field| field.kind == SettingKind::Url)
        .filter_map(|field| Some(glob::Pattern::escape(settings.url(&field.key)?.host_str()?)));
    package
        .allowed_hosts
        .iter()
        .cloned()
        .chain(url_hosts)
        .collect()
}

/// `settings` as the Extism config a Plugin reads them from, each value as text: a bool as
/// `true` or `false`, a number in decimal, Library ids in decimal joined by `,` (empty for none).
pub(super) fn extism_config(schema: &SettingsSchema, settings: &Settings) -> Vec<(String, String)> {
    schema
        .fields
        .iter()
        .filter_map(|field| {
            let text = match settings.get(&field.key)? {
                SettingValue::Bool(on) => on.to_string(),
                SettingValue::Integer(number) => number.to_string(),
                SettingValue::Text(text) => text.clone(),
                SettingValue::Libraries(ids) => {
                    ids.iter().map(i64::to_string).collect::<Vec<_>>().join(",")
                }
            };
            Some((field.key.clone(), text))
        })
        .collect()
}

/// Checks that this Server's Plugin contract version meets `wants`, a Plugin's semver requirement.
fn check_contract(wants: &str) -> Result<(), Refused> {
    let requirement = VersionReq::parse(wants).map_err(|error| Refused::BadRequirement {
        wants: wants.to_owned(),
        error,
    })?;
    // every_bundled_plugin_has_the_exports_its_manifests_points_need fails if this stops parsing.
    if Version::parse(CONTRACT_VERSION).is_ok_and(|server| requirement.matches(&server)) {
        Ok(())
    } else {
        Err(Refused::OutOfRange {
            wants: wants.to_owned(),
        })
    }
}

/// Checks that no host in a Manifest's `allowed_hosts` has a port, which Extism would never match.
pub(super) fn check_allowed_hosts(hosts: &[String]) -> Result<(), Refused> {
    match hosts.iter().find(|host| host.contains(':')) {
        Some(host) => Err(Refused::HostWithPort(host.clone())),
        None => Ok(()),
    }
}

/// Checks that each id namespace a Manifest's Fulfillers name is one, by `ExternalId`'s rule.
fn check_namespaces(package: &wire::Manifest) -> Result<(), Refused> {
    let fields = [
        ("movie_id_namespaces", &package.movie_id_namespaces),
        ("show_id_namespaces", &package.show_id_namespaces),
    ];
    for (field, namespaces) in fields {
        if let Some(namespace) = namespaces.iter().find(|namespace| !is_namespace(namespace)) {
            return Err(Refused::BadNamespace {
                field,
                namespace: namespace.clone(),
            });
        }
    }
    Ok(())
}

/// Checks that a Manifest naming `TitleSourceService` among `points` names the namespace it keeps
/// titles under, and that each other namespace it looks titles up by is one.
fn check_title_namespace(package: &wire::Manifest, points: &[&Point]) -> Result<(), Refused> {
    let names_title_source = points
        .iter()
        .any(|point| point.service == services::TITLE_SOURCE);
    if !names_title_source {
        return Ok(());
    }
    if !is_namespace(&package.title_namespace) {
        return Err(Refused::BadNamespace {
            field: "title_namespace",
            namespace: package.title_namespace.clone(),
        });
    }
    let lookup = &package.title_lookup_namespaces;
    if let Some(namespace) = lookup.iter().find(|namespace| !is_namespace(namespace)) {
        return Err(Refused::BadNamespace {
            field: "title_lookup_namespaces",
            namespace: namespace.clone(),
        });
    }
    Ok(())
}

/// Checks that a Manifest naming `AutomationService` among `points` ticks no more often than
/// [`MIN_TICK_EVERY`]; one that names no tick, 0, ticks too often too.
fn check_tick_every(package: &wire::Manifest, points: &[&Point]) -> Result<(), Refused> {
    if names(points, services::AUTOMATION)
        && u64::from(package.tick_every_seconds) < MIN_TICK_EVERY.as_secs()
    {
        return Err(Refused::TickTooOften {
            seconds: package.tick_every_seconds,
        });
    }
    Ok(())
}

/// Whether `points` include `service`'s.
fn names(points: &[&Point], service: &str) -> bool {
    points.iter().any(|point| point.service == service)
}

/// Whether `namespace` is one by `ExternalId`'s rule. Empty isn't.
fn is_namespace(namespace: &str) -> bool {
    // Any value will do: only the namespace is being checked.
    !matches!(
        ExternalId::new(namespace, "_"),
        Err(InvalidId::Namespace(_))
    )
}

pub(super) fn settings_schema(package: &wire::Manifest) -> Result<SettingsSchema, Refused> {
    schema(&package.settings)
}

/// `fields` as the Kernel reads them, or why it refuses the Plugin they are from.
fn schema(fields: &[wire::SettingField]) -> Result<SettingsSchema, Refused> {
    let fields = fields.iter().map(setting_field).collect::<Result<_, _>>()?;
    Ok(SettingsSchema { fields })
}

/// `field` as the Kernel reads it, or why it refuses the Plugin it is from.
fn setting_field(field: &wire::SettingField) -> Result<SettingField, Refused> {
    let kind = field
        .kind
        .as_known()
        .and_then(contract::setting_kind)
        .ok_or_else(|| Refused::UnknownSettingKind {
            key: field.key.clone(),
        })?;
    let default = field
        .default_value
        .as_option()
        .and_then(contract::setting_value);
    if default.as_ref().is_some_and(|default| !default.is(kind)) {
        return Err(Refused::DefaultWrongKind {
            key: field.key.clone(),
            kind,
        });
    }
    let field = SettingField {
        key: field.key.clone(),
        label: field.label.clone(),
        description: field.description.clone(),
        kind,
        required: field.required,
        default,
        min: field.min_value,
        max: field.max_value,
    };
    check_bounds(&field)?;
    Ok(field)
}

/// Refuses bounds on a setting that isn't [`SettingKind::Integer`], a `min` over its `max`, or a
/// default outside them.
fn check_bounds(field: &SettingField) -> Result<(), Refused> {
    let key = || field.key.clone();
    if field.kind != SettingKind::Integer && (field.min.is_some() || field.max.is_some()) {
        return Err(Refused::BoundsOnAnotherKind { key: key() });
    }
    if let (Some(min), Some(max)) = (field.min, field.max)
        && min > max
    {
        return Err(Refused::BoundsCrossed {
            key: key(),
            min,
            max,
        });
    }
    if let Some(SettingValue::Integer(default)) = field.default
        && !field.within_bounds(default)
    {
        return Err(Refused::DefaultOutOfBounds { key: key() });
    }
    Ok(())
}

#[cfg(test)]
#[path = "load_tests.rs"]
mod tests;
