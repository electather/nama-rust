## Built-in Plugins

- **Load the `rust-crates` skill** before you change a Plugin. It says how to write, check and review Rust here.
- A Built-in Plugin depends on no workspace crate except `nama-plugin-api` and `nama-proto`. An Installed Plugin (a `cdylib`, such as `radarr` or `telegram`) depends on none except `nama-pdk` and the shared libraries here, such as `servarr`, and a shared library follows the same rule. That way the compiler keeps them out of Kernel internals (ADR 0006). A Plugin never depends on another Plugin, and nothing here depends on `nama-server`.
- **Every Plugin ships a logo**: a `logo.svg` next to its `Cargo.toml`, embedded with `include_str!` as its Manifest's `logo_svg`. It must read on a light and a dark tile alike, since an image can't follow the colour scheme.
