## Built-in Plugins

- A Built-in Plugin depends on no workspace crate except `nama-plugin-api` and `nama-proto`. That way the compiler keeps it out of Kernel internals (ADR 0006). Never add `nama-server` as a dependency.
