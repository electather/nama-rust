//! Build script for the web app.
//!
//! With the `embed` feature, it embeds the built web app so a release is one binary (ADR 0004,
//! ADR 0105). It writes `web_assets.rs` to `OUT_DIR` for `src/lib.rs`: `SHELL`, the app's
//! `index.html`, and `asset`, which looks up every file `vp build` wrote to `web/dist` by its URL
//! path.

use std::{
    env,
    error::Error,
    fmt::Write as _,
    fs,
    path::{Path, PathBuf},
};

fn main() -> Result<(), Box<dyn Error>> {
    if env::var_os("CARGO_FEATURE_EMBED").is_none() {
        return Ok(());
    }
    let dist = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?).join("../../web/dist");
    println!("cargo:rerun-if-changed={}", dist.display());
    let dist = dist.canonicalize().map_err(|err| {
        format!(
            "nama-web embeds {}, which can't be read ({err}): run `vp build` in web/ first",
            dist.display()
        )
    })?;
    let mut files = Vec::new();
    list_files(&dist, &mut files)?;
    let shell = dist.join("index.html");
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
        let url_path = format!("/{}", utf8(file.strip_prefix(&dist)?)?.replace('\\', "/"));
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
