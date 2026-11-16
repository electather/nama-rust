//! The test Plugin (`crates/nama-test-plugin`), built from source for wasm32.

use std::{fs::File, io, path::Path, process::Command};

/// The test Plugin's `.wasm`, built for wasm32 with `features`. A Plugin that can't be built fails
/// the test that needs it.
pub(crate) fn build(features: &str) -> Vec<u8> {
    let built = built(features);
    // An assert rather than an unwrap: a unit test includes this file too, and clippy allows an
    // unwrap there but not in a test target's support file, so no one lint expectation fits both.
    assert_eq!(
        built.as_ref().err().map(ToString::to_string),
        None,
        "building the test Plugin failed"
    );
    built.unwrap_or_default()
}

fn built(features: &str) -> io::Result<Vec<u8>> {
    // One target directory for every feature set, so its dependencies build once: with one each,
    // every test process rebuilt them all in release at once, past nextest's timeout. The `.wasm`
    // lands at the same path whatever the features, so the build and the read hold one lock
    // across test processes. CI builds the dependencies into this directory first (`test` job in
    // ci.yml), so the first test here doesn't compile them inside its timeout. A unit test has no
    // CARGO_TARGET_TMPDIR, so it builds into the workspace's own `target/tmp`.
    let tmp_dir = option_env!("CARGO_TARGET_TMPDIR")
        .unwrap_or(concat!(env!("CARGO_MANIFEST_DIR"), "/../../target/tmp"));
    let target_dir = Path::new(tmp_dir).join("test-plugin");
    std::fs::create_dir_all(&target_dir)?;
    let lock = File::create(target_dir.join("build.lock"))?;
    lock.lock()?;
    let status = Command::new(env!("CARGO"))
        .args([
            "build",
            "--release",
            "--locked",
            "--package",
            "nama-test-plugin",
        ])
        .args(["--target", "wasm32-unknown-unknown", "--features", features])
        .arg("--target-dir")
        .arg(&target_dir)
        .status()?;
    if !status.success() {
        return Err(io::Error::other(format!(
            "cargo build exited with {status}"
        )));
    }
    std::fs::read(target_dir.join("wasm32-unknown-unknown/release/nama_test_plugin.wasm"))
}
