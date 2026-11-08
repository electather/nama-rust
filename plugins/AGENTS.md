## Built-in Plugins

- **Load the `rust-crates` skill** before you change a Plugin. It says how to write, check and review Rust here.
- A Built-in Plugin depends on no workspace crate except `nama-plugin-api` and `nama-proto`, and an Installed Plugin (a `cdylib`, such as `radarr` or `telegram`) on none except `nama-pdk`. That way the compiler keeps it out of Kernel internals (ADR 0006). Never add `nama-server` as a dependency.
