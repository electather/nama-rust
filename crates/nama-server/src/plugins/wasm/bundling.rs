//! Which folders under `plugins/` the Server bundles, by the rule in `plugins/AGENTS.md`.
//!
//! std only: `build.rs` and `tests/server/plugin_service.rs` include this file too, so every item
//! here is one each of them uses.

use std::{fs, io, path::Path};

/// The names of the folders under `plugins` the Server bundles, sorted.
///
/// # Errors
///
/// If `plugins` can't be read, or a bundled folder's name isn't valid UTF-8.
pub(crate) fn bundled(plugins: &Path) -> io::Result<Vec<String>> {
    let mut names = Vec::new();
    for entry in fs::read_dir(plugins)? {
        let path = entry?.path();
        let Ok(cargo_toml) = fs::read_to_string(path.join("Cargo.toml")) else {
            continue;
        };
        if bundles(&cargo_toml) {
            let name = path.file_name().and_then(|name| name.to_str());
            names.push(
                name.ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("{} isn't valid UTF-8", path.display()),
                    )
                })?
                .to_owned(),
            );
        }
    }
    names.sort_unstable();
    Ok(names)
}

/// Whether a folder with this `Cargo.toml` is a Plugin the Server bundles.
fn bundles(cargo_toml: &str) -> bool {
    // ponytail: a line scan, not a TOML parser; it misses these keys written any other way
    // (spacing, inline tables). Parse with a toml crate if a manifest needs that.
    let lines: Vec<_> = cargo_toml.lines().map(str::trim).collect();
    lines.contains(&r#"crate-type = ["cdylib"]"#) && !lines.contains(&"bundled = false")
}

#[cfg(test)]
#[path = "bundling_tests.rs"]
mod tests;
