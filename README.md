# Nama

A self-hosted media server and request manager, with an optional central Hub for identity and Server discovery. [`GLOSSARY.md`](GLOSSARY.md) defines the terms used throughout, and [`docs/adr/`](docs/adr) records the decisions behind the design.

## Layout

| Path | What it is |
|---|---|
| [`crates/nama-server`](crates/nama-server) | The Server: owns Libraries, streams media and handles Requests |
| [`crates/nama-hub`](crates/nama-hub) | The Hub: Accounts and Server discovery |
| [`crates/nama-plugin-api`](crates/nama-plugin-api) | Extension Point traits through which Plugins take part in a Server |
| [`crates/nama-pdk`](crates/nama-pdk) | Plugin development kit for Installed Plugins |
| [`crates/nama-proto`](crates/nama-proto) | Rust types and ConnectRPC stubs generated from `proto/` |
| [`plugins/`](plugins) | Built-in Plugins |
| [`proto/`](proto) | The Protobuf API schema |

## Checks

CI ([`.github/workflows/ci.yml`](.github/workflows/ci.yml)) runs the following, and every warning fails it:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo nextest run --workspace --locked
cargo test --workspace --doc --locked
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked
cargo machete
cargo deny check
```

It also checks that the generated code and the sqlx query data are current. If you change `proto/`, see [`crates/nama-proto/README.md`](crates/nama-proto/README.md). If you change a query or a migration, see [`crates/nama-server/README.md`](crates/nama-server/README.md).

## License

The Server, Hub, `nama-plugin-api` and Built-in Plugins are AGPL-3.0-only ([`LICENSE`](LICENSE)). The contracts that others build against, `nama-proto`, `nama-pdk` and `proto/`, are MIT OR Apache-2.0 (ADR 0009). Each crate's `Cargo.toml` states its license.
