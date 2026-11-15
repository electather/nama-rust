//! The Installed Plugins the Server bundles.
//!
//! `BUNDLED` holds each one's `.wasm` by the name it is listed under if refused. `build.rs` builds
//! them and writes it, from the folders under `plugins/` by the rule in `plugins/AGENTS.md`.

include!(concat!(env!("OUT_DIR"), "/bundled.rs"));
