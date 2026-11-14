//! The Installed Plugins the Server bundles.

/// The Installed Plugins bundled in the Server, each by the name it is listed under if refused.
/// `build.rs` builds them.
pub(super) const BUNDLED: &[(&str, &[u8])] = &[
    ("fanart.wasm", include_bytes!(env!("FANART_WASM"))),
    ("radarr.wasm", include_bytes!(env!("RADARR_WASM"))),
    ("sonarr.wasm", include_bytes!(env!("SONARR_WASM"))),
    ("telegram.wasm", include_bytes!(env!("TELEGRAM_WASM"))),
    ("tvdb.wasm", include_bytes!(env!("TVDB_WASM"))),
];
