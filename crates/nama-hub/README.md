# nama-hub

The Nama Hub: Accounts and Server discovery. It serves the Hub API, `proto/nama/hub/v1/` (generated into [`nama-hub-proto`](../nama-hub-proto)), and keeps its data in PostgreSQL 18 with pgvector (ADR 0005).

## Running it

It reads its config from environment variables only:

| Variable | What it sets |
|---|---|
| `NAMA_HUB_DATABASE_URL` | The PostgreSQL database, as a `postgres://` URL. Required. |
| `NAMA_HUB_LISTEN` | The address to listen on. `0.0.0.0:7981` by default. |

Each release, prereleases aside, is also a Docker image for amd64 and arm64, `ghcr.io/electather/nama-hub`, with the Server's version; it listens on 7981 as user 1000.

At startup it applies every migration the database hasn't had, in one transaction. Set `OTEL_EXPORTER_OTLP_ENDPOINT` to export its traces and logs, as on the Server (root `README.md`, "Diagnostics").

[`compose.yaml`](compose.yaml) runs a local PostgreSQL in the image CI uses, with `docker compose` or `podman compose`, from this folder:

```sh
docker compose up -d
export NAMA_HUB_DATABASE_URL=postgres://nama:nama@localhost:5432/nama_hub
cargo run -p nama-hub
```

## Tests

The tests need that PostgreSQL. `#[sqlx::test]` reads `DATABASE_URL`, and makes a database of its own for each test:

```sh
DATABASE_URL=$NAMA_HUB_DATABASE_URL cargo test -p nama-hub
```

## Adding a migration

A schema change goes in a new migration named by its UTC creation time, from the workspace root:

```sh
sqlx migrate add --timestamp --source crates/nama-hub/migrations <name>
```

## Regenerating `.sqlx/` after a query or migration change

The Hub's queries are checked at compile time against the query data in `.sqlx/` at the workspace root, which the Server's share. `sqlx.toml` here has the Hub's read `NAMA_HUB_DATABASE_URL`, and the Server's read `DATABASE_URL`, so regenerating covers both. With the PostgreSQL above running, from the workspace root:

```sh
# Same minor version as the sqlx crate; CI's `rust-check` job installs the same.
cargo install --locked sqlx-cli@0.9.0 --no-default-features --features sqlite,postgres,rustls
export DATABASE_URL=sqlite:///tmp/nama-dev.db
sqlx database create
sqlx migrate run --source crates/nama-server/migrations
sqlx migrate run --source crates/nama-hub/migrations --database-url "$NAMA_HUB_DATABASE_URL"
cargo sqlx prepare --workspace -- --all-targets
```

Commit the changes under `.sqlx/`. CI's `rust-check` job fails if they're stale.
