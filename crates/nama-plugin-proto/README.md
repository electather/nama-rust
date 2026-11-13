# nama-plugin-proto

Rust types for the Plugin contract, generated from [`proto/nama/plugin/`](../../proto/nama/plugin) (ADR 0041). `nama.plugin.v1` is the module `nama_plugin_proto::nama::plugin::v1`, holding the messages the Kernel and its Installed Plugins exchange, such as the `Manifest`. `nama-pdk` re-exports this crate, so a Plugin depends on `nama-pdk` alone.

`CONTRACT_VERSION` is the Plugin contract's version, which an Installed Plugin's `Manifest.contract` requirement is matched against. It has a number of its own, separate from this crate's version in `Cargo.toml`; `AGENTS.md` says when it changes.

The contract is more than the messages: `CONTRACT_VERSION` also covers what the Kernel gives an Installed Plugin at run time, below.

## What the Kernel gives a Plugin

The Server sets this up in [`crates/nama-server/src/plugins/installed.rs`](../nama-server/src/plugins/installed.rs), mostly in `compile`, for every call into an Installed Plugin.

- **A clock.** The function `now` in the Wasm import namespace `nama` takes no arguments and returns an `i64`: the milliseconds since the Unix epoch by the Server's clock. A Plugin needs it because `SystemTime::now()` panics on `wasm32-unknown-unknown`.
- **128 MiB of memory per call**, counting the Plugin's own memory and what Extism keeps for it. A call past it fails and counts as a crash.
- **5 seconds per call.** A call that runs longer fails and counts as a crash. Three crashes in a row disable the Plugin.
- **Settings as text.** Each setting the admin set is in the Extism config under its key, as text: a bool as `true` or `false`, a number in decimal.
- **HTTP to named hosts only.** A Plugin may send HTTP requests only to the hosts its Manifest's `allowed_hosts` names and the host of each of its URL settings.
- **No HTTP response headers.** The Kernel doesn't pass a response's headers to the Plugin: `compile` leaves Extism's `with_http_response_headers` off.

## Generated code is checked in

The code under `src/generated/` is written by `buf generate` and committed, for the reasons in [`crates/nama-proto/README.md`](../nama-proto/README.md). Don't edit it by hand. To regenerate after a schema change, install the tools as that README says, then run `buf generate` in `proto/`; one run writes both crates and the web app's code.

The contract imports nothing from the client API, `proto/nama/v1/`. `tests/stands_alone.rs` fails if it does.
