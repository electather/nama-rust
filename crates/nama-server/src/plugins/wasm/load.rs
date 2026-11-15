//! Reading, compiling and checking an Installed Plugin's Package.

use std::{
    fs, io, mem,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use buffa::{
    DecodeError, Message,
    encoding::{self, Tag, WireType},
    types,
};
use extism::{CompiledPlugin, CurrentPlugin, PluginBuilder, UserData, Val, ValType, Wasm};
use nama_plugin_api::{
    Manifest, PluginId,
    metadata::{ExternalId, InvalidId},
    settings::{SettingField, SettingKind, SettingValue, Settings, SettingsSchema},
};
use nama_plugin_proto::{CONTRACT_VERSION, nama::plugin::v1 as wire, services};
use semver::{Version, VersionReq};
use tracing::error;

use super::{
    Installed, Point, Refused,
    bundled::BUNDLED,
    check_exports, contract, points,
    services::{CHECK_SETTINGS, known_exports},
};
use crate::plugins::{contain_sync, kept_logo, registry::personal_settings_refusal};

// Changing it changes what the Kernel gives a Plugin: see `crates/nama-plugin-proto/AGENTS.md`.
/// How long one call into an Installed Plugin may run, so one that never returns can't hold up
/// the start, its Observer's task or a settings check.
pub(super) const CALL_TIMEOUT: Duration = Duration::from_secs(5);

// Changing it changes what the Kernel gives a Plugin: see `crates/nama-plugin-proto/AGENTS.md`.
/// How much memory one call into an Installed Plugin may use, in 64 KiB pages: the guest's own and
/// what Extism keeps for it, so one Plugin can't take the Server down.
// ponytail: one cap for all; let a Manifest ask for more once a Plugin needs it.
pub(super) const MEMORY_MAX_PAGES: u32 = 2048;

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
        let mut plugin = extism::Plugin::new_from_compiled(&compiled).map_err(Refused::Manifest)?;
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
    check_exports(&points, |export| exported.contains(&export))?;
    manifest(package, &points)
}

/// What `package`, naming `points`, says about its Plugin, as the Kernel reads it. Its logo is
/// taken out, so the package kept with the Plugin never holds one the Server dropped.
fn manifest(package: &mut wire::Manifest, points: &[&Point]) -> Result<Manifest, Refused> {
    let settings = settings_schema(package)?;
    let personal_settings = schema(&package.personal_settings)?;
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
    };
    match personal_settings_refusal(&manifest) {
        Some(reason) => Err(Refused::PersonalSettings(reason)),
        None => Ok(manifest),
    }
}

// Changing it changes what the Kernel gives a Plugin: see `crates/nama-plugin-proto/AGENTS.md`.
/// Compiles `wasm`, letting it send HTTP requests to `hosts`, read `config` and call [`now`].
pub(super) fn compile(
    wasm: &[u8],
    cache_config: &Path,
    hosts: Vec<String>,
    config: Vec<(String, String)>,
) -> Result<CompiledPlugin, extism::Error> {
    let manifest = extism::Manifest::new([Wasm::data(wasm.to_vec())])
        .with_timeout(CALL_TIMEOUT)
        .with_memory_max(MEMORY_MAX_PAGES)
        .with_allowed_hosts(hosts.into_iter())
        .with_config(config.into_iter());
    CompiledPlugin::new(
        PluginBuilder::new(manifest)
            .with_cache_config(cache_config)
            .with_function_in_namespace("nama", "now", [], [ValType::I64], UserData::new(()), now),
    )
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
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_millis());
    out.fill(Val::I64(i64::try_from(millis).unwrap_or(i64::MAX)));
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
/// `true` or `false`, a number in decimal.
pub(super) fn extism_config(schema: &SettingsSchema, settings: &Settings) -> Vec<(String, String)> {
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
    let fields = fields
        .iter()
        .map(|field| {
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
#[path = "load_tests.rs"]
mod tests;
