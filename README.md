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
| [`web/`](web) | The web app the Server serves |
| [`proto/`](proto) | The Protobuf API schema |
| [`spikes/`](spikes) | Throwaway prototypes, not workspace members |

## Metadata from TMDB

A Server Matches the files in its Libraries to Movies and Shows using the built-in TMDB Plugin ([`plugins/tmdb`](plugins/tmdb)). It needs a TMDB API key, which you can get for free from [themoviedb.org](https://www.themoviedb.org/settings/api). Put it in the Server's `nama.toml`:

```toml
[plugins.tmdb]
api_key = "your TMDB API key"
# language = "en-US"   # the language titles and overviews are fetched in
```

Without a key the Plugin isn't enabled and every file stays Unmatched.

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

A `web` job runs the web app's checks ([`web/README.md`](web/README.md), "Checks"). The test job then builds the web app and runs `cargo test -p nama-server --features web --test web` against it.

## License

The Server, Hub, `nama-plugin-api` and Built-in Plugins are AGPL-3.0-only ([`LICENSE`](LICENSE)). The contracts that others build against, `nama-proto`, `nama-pdk` and `proto/`, are MIT OR Apache-2.0 (ADR 0009). Each crate's `Cargo.toml` states its license.

## Attribution

This product uses the TMDB API but is not endorsed or certified by TMDB. Metadata and images are provided by [The Movie Database (TMDB)](https://www.themoviedb.org).
