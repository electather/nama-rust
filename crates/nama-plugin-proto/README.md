# nama-plugin-proto

Rust types for the Plugin contract, generated from [`proto/nama/plugin/`](../../proto/nama/plugin) (ADR 0041). `nama.plugin.v1` is the module `nama_plugin_proto::nama::plugin::v1`, holding the messages the Kernel and its Installed Plugins exchange, such as the `Manifest`. `nama-pdk` re-exports this crate, so a Plugin depends on `nama-pdk` alone.

`CONTRACT_VERSION` is the Plugin contract's version, which an Installed Plugin's `Manifest.nama_api` requirement is matched against. It has a number of its own, separate from this crate's version in `Cargo.toml`; `AGENTS.md` says when it changes.

Besides the messages, the Kernel gives an Installed Plugin the `now` function in the `nama` namespace, which returns the time, and up to 128 MiB of memory per call.

## Generated code is checked in

The code under `src/generated/` is written by `buf generate` and committed, for the reasons in [`crates/nama-proto/README.md`](../nama-proto/README.md). Don't edit it by hand. To regenerate after a schema change, install the tools as that README says, then run `buf generate` in `proto/`; one run writes both crates and the web app's code.

The contract imports nothing from the client API, `proto/nama/v1/`. `tests/stands_alone.rs` fails if it does.
