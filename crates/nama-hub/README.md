# nama-hub

The Nama Hub: Accounts and Server discovery. It serves the Hub API, `proto/nama/hub/v1/` (generated into [`nama-hub-proto`](../nama-hub-proto)), and keeps its data in PostgreSQL 18 with pgvector (ADR 0005).

## Running it

It reads its config from environment variables only:

| Variable | What it sets |
|---|---|
| `NAMA_HUB_DATABASE_URL` | The PostgreSQL database, as a `postgres://` URL. Required. |
| `NAMA_HUB_LISTEN` | The address to listen on. `0.0.0.0:7981` by default. |
| `NAMA_HUB_SIGNING_KEYS` | The Ed25519 private keys it signs the tokens Servers check with, as PKCS#8 PEM blocks one after another, such as `openssl genpkey -algorithm ed25519` writes. Required. The first signs, and all are published at `/.well-known/jwks.json`: to rotate, put a new key first, and remove the old one once Servers have fetched the new set (ADR 0098). |
| `NAMA_HUB_TRUSTED_PROXIES` | The reverse proxies whose `X-Forwarded-For` and `X-Forwarded-Proto` it believes, such as `["10.0.0.0/8"]` (ADR 0018). None by default, so rate limits count the peer's address and the session cookie is `Secure` only behind a listed proxy that says HTTPS. |
| `NAMA_HUB_RESEND_API_KEY` | The [Resend](https://resend.com) API key it sends mail with, such as Sign-in Links. Unset by default, so it sends none and its sign-in page doesn't offer Sign-in Links, and no Account can prove its email (ADR 0100). Use a Resend test key while developing. |
| `NAMA_HUB_MAIL_FROM` | The address its mail comes from, such as `Nama Hub <hub@example.com>`, on a domain verified with Resend. Required with a Resend key. |
| `NAMA_HUB_PUBLIC_URL` | Its address as people reach it, such as `https://hub.example.com`, which the links in its mail lead to. Required with a Resend key: it never takes the address from a request. |

Each release, prereleases aside, is also a Docker image for amd64 and arm64, `ghcr.io/electather/nama-hub`, with the Server's version; it listens on 7981 as user 1000.

With the `web` feature, which releases turn on, it also serves its web pages, the Hub web app in `web/hub/` (`web/README.md`): build them first with `vp -C hub build` in `web/`. Without it, it serves only the API, and `vp -C hub dev` in `web/` serves the pages against it.

At startup it applies every migration the database hasn't had, in one transaction. Set `OTEL_EXPORTER_OTLP_ENDPOINT` to export its traces and logs, as on the Server (root `README.md`, "Diagnostics").

[`compose.yaml`](compose.yaml) runs a local PostgreSQL in the image CI uses, with `docker compose` or `podman compose`, from this folder:

```sh
docker compose up -d
export NAMA_HUB_DATABASE_URL=postgres://nama:nama@localhost:5432/nama_hub
export NAMA_HUB_SIGNING_KEYS="$(openssl genpkey -algorithm ed25519)"
cargo run -p nama-hub
```

A Server Links to it through its Hub Plugin, which calls the Hub that `NAMA_HUB_URL` names on the Server, such as `NAMA_HUB_URL=http://localhost:7981 cargo run -p nama-server`. Linking needs an Account whose email a Sign-in Link proved (ADR 0100).

## Importing Wikidata

The Hub knows films and series, and which TMDB, IMDb and TVDB ids name each, from Wikidata's JSON dump, which Wikidata publishes weekly (ADR 0095). `nama-hub import-wikidata` reads the decompressed dump on stdin and replaces the titles with those in it, in one transaction: the Hub keeps answering from the last import meanwhile, and a dump cut short, which ends before its closing `]`, changes nothing. Run it weekly from the host's scheduler, as its own process, never inside the serving one (ADR 0096), with the same `NAMA_HUB_DATABASE_URL`, after the serving Hub has migrated the database:

```sh
curl -fsS https://dumps.wikimedia.org/wikidatawiki/entities/latest-all.json.gz \
  | gzip -d | nama-hub import-wikidata
# or, from the Docker image:
curl -fsS https://dumps.wikimedia.org/wikidatawiki/entities/latest-all.json.gz \
  | gzip -d | docker run --rm -i -e NAMA_HUB_DATABASE_URL ghcr.io/electather/nama-hub import-wikidata
```

The dump is over 100 GiB compressed, so an import takes hours; it streams, holding nothing on disk. Its transaction holds the titles' tables all that time, so deploy a Hub whose migration changes them between imports: its startup would wait for the import to finish.

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
