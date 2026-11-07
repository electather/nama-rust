//! Installed Plugins (ADR 0006): `.wasm` files an admin drops into `<data_dir>/plugins/`.

use std::{
    fs, io,
    path::{Path, PathBuf},
    time::Duration,
};

use buffa::Message;
use extism::{PluginBuilder, Wasm};
use nama_proto::{NAMA_API_VERSION, nama::plugin::v1::Manifest};
use semver::{Version, VersionReq};
use thiserror::Error;
use tracing::{error, info, warn};

use super::contain_sync;

/// How long a Plugin's `manifest()` may run, so one that never returns can't hold up the start.
const MANIFEST_TIMEOUT: Duration = Duration::from_secs(5);

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
    #[error("its manifest() export failed: {0:#}")]
    Manifest(extism::Error),
    #[error("loading it panicked: {0}")]
    Panicked(String),
    #[error("its manifest() returned something that isn't a Manifest: {0}")]
    Decode(buffa::DecodeError),
    #[error("its nama-api requirement {wants:?} isn't a semver requirement: {error}")]
    BadRequirement { wants: String, error: semver::Error },
    #[error("it wants nama-api {wants}, and this Server has {}", NAMA_API_VERSION)]
    OutOfRange { wants: String },
}

/// Reads the Manifest of each `.wasm` file in `<data_dir>/plugins/`, in name order, and logs the
/// id and version of each Installed Plugin, or why the Server refused it. Enables none of them.
///
/// Blocks while it compiles each Plugin. wasmtime caches what it compiles under
/// `<data_dir>/wasmtime/`, not in the home directory.
pub fn log_installed(data_dir: &Path) {
    let files = match wasm_files(&data_dir.join("plugins")) {
        Ok(files) if files.is_empty() => return,
        Ok(files) => files,
        Err(err) => {
            error!(
                "could not list the Installed Plugins in {}: {err}",
                data_dir.display()
            );
            return;
        }
    };
    let cache_config = match cache_config(data_dir) {
        Ok(cache_config) => cache_config,
        Err(err) => {
            error!(
                "could not set up wasmtime's compile cache in {}: {err}",
                data_dir.display()
            );
            return;
        }
    };
    for file in files {
        match read_manifest(&file, &cache_config) {
            Ok(manifest) => info!(
                file = %file.display(),
                id = %manifest.id,
                version = %manifest.version,
                "found Installed Plugin"
            ),
            Err(refused) => warn!(file = %file.display(), "refused Installed Plugin: {refused}"),
        }
    }
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
fn cache_config(data_dir: &Path) -> io::Result<PathBuf> {
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
fn read_manifest(file: &Path, cache_config: &Path) -> Result<Manifest, Refused> {
    let wasm = fs::read(file).map_err(Refused::Read)?;
    // Extism would also take WASM's text format, and its parse errors span many lines.
    if !wasm.starts_with(WASM_MAGIC) {
        return Err(Refused::NotWasm);
    }
    let builder = PluginBuilder::new(
        extism::Manifest::new([Wasm::data(wasm)]).with_timeout(MANIFEST_TIMEOUT),
    )
    .with_cache_config(cache_config);
    let bytes = contain_sync(|| {
        let mut plugin = builder.build().map_err(Refused::Load)?;
        plugin
            .call::<(), Vec<u8>>("manifest", ())
            .map_err(Refused::Manifest)
    })
    .map_err(Refused::Panicked)??;
    let manifest = Manifest::decode_from_slice(&bytes).map_err(Refused::Decode)?;
    check_nama_api(&manifest.nama_api)?;
    Ok(manifest)
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
