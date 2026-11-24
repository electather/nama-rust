//! Build script for the Server.
//!
//! It builds the Installed Plugins the Server bundles (ADR 0003) for wasm32, and writes
//! `bundled.rs` to `OUT_DIR` for `src/plugins/wasm/bundled.rs`: `BUNDLED`, each one's `.wasm` by
//! its folder's name. `bundling.rs` picks those folders.
//!
//! With the `web` feature, it also embeds the built web app so a release is one binary (ADR 0004).
//! It writes `web_assets.rs` to `OUT_DIR` for `src/web.rs`: `SHELL`, the app's `index.html`, and
//! `asset`, which looks up every file `vp build` wrote to `web/dist` by its URL path.

use std::{
    env,
    error::Error,
    fmt::Write as _,
    fs, io,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

#[path = "src/plugins/wasm/bundling.rs"]
mod bundling;

fn main() -> Result<(), Box<dyn Error>> {
    // `sqlx::migrate!` embeds migrations/ at compile time; rebuild when it changes.
    println!("cargo:rerun-if-changed=migrations");
    build_bundled()?;
    if env::var_os("CARGO_FEATURE_WEB").is_none() {
        return Ok(());
    }
    let client = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?).join("../../web/dist");
    println!("cargo:rerun-if-changed={}", client.display());
    let client = client.canonicalize().map_err(|err| {
        format!(
            "the `web` feature embeds {}, which can't be read ({err}): run `vp build` in web/ first",
            client.display()
        )
    })?;

    let mut files = Vec::new();
    list_files(&client, &mut files)?;
    let shell = client.join("index.html");
    if !files.contains(&shell) {
        return Err(format!("{} is missing: run `vp build` in web/", shell.display()).into());
    }

    // `{:?}` writes a `&str` as a Rust string literal, with any `\` or `"` escaped.
    let mut code = format!(
        "const SHELL: &[u8] = include_bytes!({:?});\n\n",
        utf8(&shell)?
    );
    // The real bundle always trips both: 200+ files and the 1 MB+ jassub wasm.
    code.push_str(
        "#[expect(clippy::too_many_lines, clippy::large_include_file, reason = \"generated from the bundle\")]\n",
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

/// Builds the [`bundling::bundled`] Plugins for wasm32 and writes `bundled.rs` to `OUT_DIR`,
/// embedding each one's `.wasm`.
fn build_bundled() -> Result<(), Box<dyn Error>> {
    let workspace = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?).join("../..");
    let names = bundling::bundled(&workspace.join("plugins"))?;
    // `plugins` covers every Plugin's folder, the shared libraries among them, and a folder added.
    for source in [
        "crates/nama-pdk",
        "crates/nama-plugin-proto",
        "plugins",
        "Cargo.toml",
        "Cargo.lock",
    ] {
        println!(
            "cargo:rerun-if-changed={}",
            workspace.join(source).display()
        );
    }
    // A target directory of its own, since this build holds the lock on the Server's. It sits in the
    // profile's folder, three up from `OUT_DIR` (`<profile>/build/nama-server-<hash>/out`), so
    // `cargo build`, `clippy`, `nextest` and each feature set share one Plugin build instead of
    // each building them again.
    let out_dir = PathBuf::from(env::var("OUT_DIR")?);
    let target_dir = out_dir
        .ancestors()
        .nth(3)
        .unwrap_or(&out_dir)
        .join("bundled-plugins");
    let mut cargo = Command::new(env::var("CARGO")?);
    cargo.args(["build", "--release", "--locked"]);
    for name in &names {
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
    let mut code = String::from("pub(super) const BUNDLED: &[(&str, &[u8])] = &[\n");
    for name in &names {
        let wasm = target_dir.join(format!(
            "wasm32-unknown-unknown/release/nama_plugin_{}.wasm",
            name.replace('-', "_")
        ));
        writeln!(
            code,
            "    ({:?}, include_bytes!({:?})),",
            format!("{name}.wasm"),
            utf8(&wasm)?
        )?;
    }
    code.push_str("];\n");
    fs::write(out_dir.join("bundled.rs"), code)?;
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
