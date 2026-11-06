# nama-server

The Nama Server: owns Libraries, streams media and handles Requests. Its database is SQLite (ADR 0005).

## Writing to the database

Every write goes through `Database::write`, which hands it to the single writer task that owns the only writable connection. Never open another writable connection. Long jobs such as a scan write in small batches so other writes and reads aren't held up. `src/db.rs` explains why.

## Regenerating `.sqlx/` after a query or migration change

Queries use `sqlx::query!` and friends, which check them at compile time against the query data checked in under `.sqlx/` at the workspace root. Builds need no database. After adding or changing a query or a migration, regenerate that data against a scratch database, from the workspace root:

```sh
# Same minor version as the sqlx crate in Cargo.toml; CI's `sqlx` job installs the same.
cargo install --locked sqlx-cli@0.9.0 --no-default-features --features sqlite,rustls
export DATABASE_URL=sqlite:///tmp/nama-dev.db
sqlx database create
sqlx migrate run --source crates/nama-server/migrations
cargo sqlx prepare --workspace -- --all-targets
```

Commit the changes under `.sqlx/`. CI's `sqlx` job runs `cargo sqlx prepare --check` and fails if they're stale. When you upgrade the `sqlx` crate, bump the `sqlx-cli` version here and in `.github/workflows/ci.yml` to the same minor version.

## Serving the web app

The `web` feature embeds the web app's build output (`web/dist/client`) in the binary and serves it: RPCs and `/media/…` are untouched, and any other `GET` gets the matching file or else `_shell.html`, so deep links work (ADR 0004). It's off by default, so building and testing the Server needs no Node. Release builds turn it on in `dist-workspace.toml`. Build the web app first (see `web/README.md`), then:

```sh
vp -C web build
cargo run -p nama-server --features web
cargo test -p nama-server --features web --test web   # the tests that need it
```
