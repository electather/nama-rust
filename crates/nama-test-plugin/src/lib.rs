//! An Installed Plugin used only in the Server's tests. Build it with
//! `cargo build -p nama-test-plugin --target wasm32-unknown-unknown`.

/// The Plugin's exports. `plugin_fn` drops their doc comments and attributes, so lints about the
/// code it generates are expected here.
#[expect(
    clippy::unnecessary_wraps,
    reason = "an export returns `FnResult` even when it can't fail"
)]
mod exports {
    use nama_pdk::{Buffa, FnResult, Manifest, extism_pdk, plugin_fn};

    // Says who the Plugin is. With the `out-of-range` feature it wants a Nama API no Server has.
    #[plugin_fn]
    pub fn manifest() -> FnResult<Buffa<Manifest>> {
        let manifest = nama_pdk::manifest("test-plugin", env!("CARGO_PKG_VERSION"));
        #[cfg(feature = "out-of-range")]
        let manifest = Manifest {
            nama_api: "<0.1".to_owned(),
            ..manifest
        };
        Ok(Buffa(manifest))
    }
}
