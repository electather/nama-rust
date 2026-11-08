//! The test Plugin (`crates/nama-test-plugin`), built from source for wasm32.

use std::{fs::File, path::Path, process::Command};

/// The test Plugin's `.wasm`, built for wasm32 with `features`.
#[expect(
    clippy::unwrap_used,
    reason = "a Plugin that can't be built fails the test that needs it"
)]
pub(crate) fn build(features: &str) -> Vec<u8> {
    // One target directory for every feature set, so its dependencies build once: with one each,
    // every test process rebuilt them all in release at once, past nextest's timeout. The `.wasm`
    // lands at the same path whatever the features, so the build and the read hold one lock
    // across test processes.
    let target_dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("test-plugin");
    std::fs::create_dir_all(&target_dir).unwrap();
    let lock = File::create(target_dir.join("build.lock")).unwrap();
    lock.lock().unwrap();
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
        .status()
        .unwrap();
    assert!(status.success(), "building the test Plugin failed");
    std::fs::read(target_dir.join("wasm32-unknown-unknown/release/nama_test_plugin.wasm")).unwrap()
}
