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
| [`crates/nama-test-plugin`](crates/nama-test-plugin) | An Installed Plugin used only in the Server's tests |
| [`crates/nama-test-report`](crates/nama-test-report) | CI tooling: records test results and reports flaky and slow tests |
| [`plugins/`](plugins) | Built-in Plugins, the Installed Plugins the Server bundles, and the libraries they share |
| [`web/`](web) | The web app the Server serves |
| [`proto/`](proto) | The Protobuf API schema |
| [`spikes/`](spikes) | Throwaway prototypes, not workspace members |

## Metadata from TMDB

A Server Matches the files in its Libraries to Movies and Shows using the built-in TMDB Plugin ([`plugins/tmdb`](plugins/tmdb)). It needs a TMDB API key, which you can get for free from [themoviedb.org](https://www.themoviedb.org/settings/api). Without a key the Plugin isn't enabled and every file stays Unmatched. An Admin sets the key on TMDB's page under Plugins in the web app, and it takes effect without a restart.

Plugin settings live in the Server's database. On its first start, fresh or after upgrading, a Server imports any `[plugins.*]` tables from an existing `nama.toml` once, such as:

```toml
[plugins.tmdb]
api_key = "your TMDB API key"
# language = "en-US"   # the language titles and overviews are fetched in
```

After that it ignores those tables, and logs that it does, so editing them changes nothing.

## Requests to Radarr and Sonarr

Approved Movie Requests go to Radarr through the Radarr Plugin ([`plugins/radarr`](plugins/radarr)), and Approved Season Requests to Sonarr through the Sonarr Plugin ([`plugins/sonarr`](plugins/sonarr)), both Installed Plugins bundled in the Server. An Admin enables each on its page under Plugins with its URL and API key, and "Test" offers its quality profiles and root folders to pick from. Sonarr finds a Show by its TVDB id, so a Show the metadata source has none for can't go to Sonarr. Asking for a whole Show also has Sonarr monitor the Show's new Seasons, until its Followed Show ends.

## Checks

CI ([`.github/workflows/ci.yml`](.github/workflows/ci.yml)) runs the following, and every warning fails it:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo clippy --package nama-test-plugin --package nama-plugin-radarr --package nama-plugin-sonarr --package nama-plugin-telegram --package nama-servarr --target wasm32-unknown-unknown --locked -- -D warnings
cargo nextest run --workspace --locked
cargo test --workspace --doc --locked
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked
cargo machete
cargo deny check bans licenses sources
```

Building the Server builds the Plugins it bundles for wasm32 (`crates/nama-server/build.rs`), and its tests build the test Plugin, so both need the `wasm32-unknown-unknown` target (`rustup target add wasm32-unknown-unknown`).

`cargo deny check advisories` fails a PR only on an advisory its own `Cargo.lock` adds, not one already on main ([`.github/scripts/advisories.sh`](.github/scripts/advisories.sh)). The [`advisories`](.github/workflows/advisories.yml) workflow checks main daily and opens a `needs-triage` issue for each advisory it finds there.

It also checks that the generated code and the sqlx query data are current. If you change `proto/`, see [`crates/nama-proto/README.md`](crates/nama-proto/README.md). If you change a query or a migration, see [`crates/nama-server/README.md`](crates/nama-server/README.md).

Its jobs run side by side on GitHub-hosted runners: `lint`, `rust-check`, `test` (split in two, on Linux for a PR and also on macOS and Windows on main), the web app's checks in `web` ([`web/README.md`](web/README.md), "Checks"), and `web-test`, which builds the web app and runs `cargo nextest run -p nama-server --features web --test web` against it. On a PR, path filters skip the side it doesn't touch: a web-only PR skips the Rust checks and the workspace tests, a Rust-only one the web checks, and one that touches neither, such as docs, runs nothing. A push to main runs everything. A PR merges into main only once `ci-ok` passes, which waits for all the others and passes when none failed.

### Flaky and slow tests

CI runs nextest with its `ci` profile ([`.config/nextest.toml`](.config/nextest.toml)), and vitest with `CI` set, so a failed test is retried twice there; locally it isn't. The [`test-report`](.github/workflows/test-report.yml) workflow then records every test's result on the `test-history` branch, summarises the run's flaky and slow tests, and opens or updates a `needs-triage` issue per flaky test (ADR 0022). To ask the history your own questions:

```sh
git fetch origin test-history
git show origin/test-history:2026-11.jsonl | jq -c 'select(.outcome != "pass")'
```

## License

The Server, Hub, `nama-plugin-api` and Built-in Plugins are AGPL-3.0-only ([`LICENSE`](LICENSE)). The contracts that others build against, `nama-proto`, `nama-pdk` and `proto/`, are MIT OR Apache-2.0 (ADR 0009). Each crate's `Cargo.toml` states its license.

## Attribution

This product uses the TMDB API but is not endorsed or certified by TMDB. Metadata and images are provided by [The Movie Database (TMDB)](https://www.themoviedb.org).
