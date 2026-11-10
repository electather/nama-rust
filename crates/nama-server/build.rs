//! Build script for the Server.
//!
//! It builds the Installed Plugins the Server bundles (ADR 0003) for wasm32, and sets
//! `<NAME>_WASM`, such as `RADARR_WASM`, to each one's `.wasm` for
//! `src/plugins/installed.rs` to embed.
//!
//! With the `web` feature, it also embeds the built web app so a release is one binary (ADR 0004).
//! It writes `web_assets.rs` to `OUT_DIR` for `src/web.rs`: `SHELL`, the prerendered
//! `_shell.html`, and `asset`, which looks up every file `vp build` wrote to `web/dist/client` by
//! its URL path.

use std::{
    env,
    error::Error,
    fmt::Write as _,
    fs, io,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

/// The Installed Plugins the Server bundles, by their folder in `plugins/`. Each is the package
/// `nama-plugin-<name>`.
const BUNDLED: &[&str] = &["radarr", "sonarr", "telegram"];

fn main() -> Result<(), Box<dyn Error>> {
    // `sqlx::migrate!` embeds migrations/ at compile time; rebuild when it changes.
    println!("cargo:rerun-if-changed=migrations");
    build_bundled()?;
    if env::var_os("CARGO_FEATURE_WEB").is_none() {
        return Ok(());
    }
    let client = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?).join("../../web/dist/client");
    println!("cargo:rerun-if-changed={}", client.display());
    let client = client.canonicalize().map_err(|err| {
        format!(
            "the `web` feature embeds {}, which can't be read ({err}): run `vp build` in web/ first",
            client.display()
        )
    })?;

    let mut files = Vec::new();
    list_files(&client, &mut files)?;
    let shell = client.join("_shell.html");
    if !files.contains(&shell) {
        return Err(format!("{} is missing: run `vp build` in web/", shell.display()).into());
    }

    // `{:?}` writes a `&str` as a Rust string literal, with any `\` or `"` escaped.
    let mut code = format!(
        "const SHELL: &[u8] = include_bytes!({:?});\n\n",
        utf8(&shell)?
    );
    code.push_str("fn asset(path: &str) -> Option<&'static [u8]> {\n    match path {\n");
    for file in &files {
        let url_path = format!("/{}", utf8(file.strip_prefix(&client)?)?.replace('\\', "/"));
        writeln!(
            code,
            "        {url_path:?} => Some(include_bytes!({:?})),",
            utf8(file)?
        )?;
    }
    code.push_str("        _ => None,\n    }\n}\n");
    fs::write(
        PathBuf::from(env::var("OUT_DIR")?).join("web_assets.rs"),
        code,
    )?;
    Ok(())
}

/// Builds the [`BUNDLED`] Plugins for wasm32 and sets `<NAME>_WASM` to each one's `.wasm`.
fn build_bundled() -> Result<(), Box<dyn Error>> {
    let workspace = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?).join("../..");
    let sources = [
        "crates/nama-pdk",
        "crates/nama-proto",
        "plugins/servarr",
        "Cargo.toml",
        "Cargo.lock",
    ];
    let plugins = BUNDLED.iter().map(|name| format!("plugins/{name}"));
    for source in plugins.chain(sources.map(str::to_owned)) {
        println!(
            "cargo:rerun-if-changed={}",
            workspace.join(source).display()
        );
    }
    // A target directory of its own: this build holds the lock on the Server's.
    // ponytail: one per OUT_DIR, so each profile and feature set builds them again; share one if
    // that gets slow.
    let target_dir = PathBuf::from(env::var("OUT_DIR")?).join("plugins");
    let mut cargo = Command::new(env::var("CARGO")?);
    cargo.args(["build", "--release", "--locked"]);
    for name in BUNDLED {
        cargo.args(["--package", &format!("nama-plugin-{name}")]);
    }
    let status = cargo
        .args(["--target", "wasm32-unknown-unknown", "--target-dir"])
        .arg(&target_dir)
        // Cargo sets these for the Server's own host build. Inherited, the host's flags would reach
        // the wasm32 build, and under `cargo clippy` the Plugins would be linted inside it, where
        // CI's own clippy run already covers them.
        .env_remove("CARGO_ENCODED_RUSTFLAGS")
        .env_remove("RUSTC_WORKSPACE_WRAPPER")
        // Cargo reads a build script's stdout as instructions.
        .stdout(Stdio::from(io::stderr()))
        .status()?;
    if !status.success() {
        return Err(
            "building the bundled Plugins for wasm32 failed: see the errors above, and if the \
             target is missing, `rustup target add wasm32-unknown-unknown`"
                .into(),
        );
    }
    for name in BUNDLED {
        let wasm = target_dir.join(format!(
            "wasm32-unknown-unknown/release/nama_plugin_{name}.wasm"
        ));
        println!(
            "cargo:rustc-env={}_WASM={}",
            name.to_uppercase(),
            utf8(&wasm)?
        );
    }
    Ok(())
}

fn list_files(dir: &Path, files: &mut Vec<PathBuf>) -> Result<(), Box<dyn Error>> {
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            list_files(&path, files)?;
        } else {
            files.push(path);
        }
    }
    Ok(())
}

/// `path` as a `&str`, which `include_bytes!` needs.
fn utf8(path: &Path) -> Result<&str, String> {
    path.to_str()
        .ok_or_else(|| format!("{} isn't valid UTF-8", path.display()))
}
