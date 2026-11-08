//! The test Plugin (`crates/nama-test-plugin`), built from source for wasm32.

use std::{path::Path, process::Command};

/// The test Plugin's `.wasm`, built for wasm32 with `features`.
#[expect(
    clippy::unwrap_used,
    reason = "a Plugin that can't be built fails the test that needs it"
)]
pub(crate) fn build(features: &str) -> Vec<u8> {
    // One target directory per feature set: the `.wasm` lands at the same path whatever the
    // features, and tests in other processes may build another set at the same time. They share
    // one build directory, so the dependencies compile once, not once per set: three cold builds
    // in a row ran a test past nextest's 60 s limit in CI.
    let test_plugin = Path::new(env!("CARGO_TARGET_TMPDIR")).join("test-plugin");
    let target_dir = test_plugin.join(if features.is_empty() {
        "default"
    } else {
        features
    });
    let status = Command::new(env!("CARGO"))
        .env("CARGO_BUILD_BUILD_DIR", test_plugin.join("build"))
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
