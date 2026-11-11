## Built-in Plugins

- **Load the `rust-crates` skill** before you change a Plugin. It says how to write, check and review Rust here.
- A Built-in Plugin depends on no workspace crate except `nama-plugin-api` and `nama-proto`. An Installed Plugin (a `cdylib`, such as `radarr` or `telegram`) depends on none except `nama-pdk` and the shared libraries here, such as `servarr`, and a shared library follows the same rule. That way the compiler keeps them out of Kernel internals (ADR 0006). A Plugin never depends on another Plugin, and nothing here depends on `nama-server`.
- A Plugin's package is `nama-plugin-<folder>`, and a shared library's is not, such as `nama-servarr`. The Server's tests tell them apart by it.
- **Every Plugin ships a logo** (a shared library doesn't): a `logo.svg` next to its `Cargo.toml`, embedded with `include_str!` as its Manifest's `logo_svg`. It must read on a light and a dark tile alike, since an image can't follow the colour scheme.
- **An Installed Plugin's `mod exports` is `#[cfg(not(test))]`**, as is `crates/nama-test-plugin`'s. Its `#[plugin_fn]`s are `#[no_mangle]`, so a native test binary keeps them and the Extism host functions they import, which MSVC refuses to leave unresolved. Its tests test what's outside `exports`.
