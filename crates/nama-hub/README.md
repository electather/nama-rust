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
| `NAMA_HUB_RESEND_API_KEY` | The [Resend](https://resend.com) API key it sends mail with, such as Sign-in Links. Unset by default, so it sends none and its sign-in page doesn't offer Sign-in Links, and no Account can prove its email (ADR 0100). While developing, use `NAMA_HUB_LOG_MAIL` instead. |
| `NAMA_HUB_LOG_MAIL` | `true` writes each email, such as a Sign-in Link, to its log instead of sending it, for developing without Resend. Off by default. It needs a `NAMA_HUB_PUBLIC_URL` on `localhost` or a loopback address, so a real deploy can't use it, and refuses a Resend key; it needs no `NAMA_HUB_MAIL_FROM`. |
| `NAMA_HUB_MAIL_FROM` | The address its mail comes from, such as `Nama Hub <hub@example.com>`, on a domain verified with Resend. Required with a Resend key. |
| `NAMA_HUB_PUBLIC_URL` | Its address as people reach it, such as `https://hub.example.com`, which the links in its mail lead to. Required with a Resend key or `NAMA_HUB_LOG_MAIL`: it never takes the address from a request. Passkeys belong to its host, so changing the host strands every passkey, and without it the Hub offers none. While developing, it's the address `vp -C hub dev` serves, such as `http://localhost:5173`: browsers allow passkeys over plain HTTP only on `localhost`. |
| `NAMA_HUB_CLOUDFLARE_API_TOKEN` | The [Cloudflare](https://cloudflare.com) API token it writes each Linked Server's names with, allowed to edit the DNS of the zone below (ADR 0104). Unset by default, so it names no Server. |
| `NAMA_HUB_CLOUDFLARE_ZONE_ID` | The id of the Cloudflare zone it names Linked Servers in. Required with a Cloudflare token. |
| `NAMA_HUB_SERVER_NAMES_ZONE` | That zone's domain, used for nothing but Server names, such as `namadirect.net` (ADR 0057). Required with a Cloudflare token. |
| `NAMA_HUB_DIAGNOSTICS_ENDPOINT` | The OTLP/HTTP endpoint the Diagnostics and Usage Analytics Linked Servers share go to, unchanged, such as `https://otlp.example.com/otlp`: each signal posts to its own path under it, as with `OTEL_EXPORTER_OTLP_ENDPOINT` (ADR 0054). Unset by default, so what Servers share is dropped. Each Linked Server may send 4 MiB a minute. |
| `NAMA_HUB_DIAGNOSTICS_HEADERS` | The headers sent with each batch to that endpoint, written as `OTEL_EXPORTER_OTLP_HEADERS` is, such as `authorization=Basic abc`. None by default. |

Each release, prereleases aside, is also a Docker image for amd64 and arm64, `ghcr.io/electather/nama-hub`, with the Server's version; it listens on 7981 as user 1000.

With the `web` feature, which releases turn on, it also serves its web pages, the Hub web app in `web/hub/` (`web/README.md`): build them first with `vp -C hub build` in `web/`. Without it, it serves only the API, and `vp -C hub dev` in `web/` serves the pages against it.

At startup it applies every migration the database hasn't had, in one transaction. Set `OTEL_EXPORTER_OTLP_ENDPOINT` to export its traces and logs, as on the Server (root `README.md`, "Diagnostics").

[`compose.yaml`](compose.yaml) runs a local PostgreSQL in the image CI uses, with `docker compose` or `podman compose`, from this folder:

```sh
docker compose up -d
export NAMA_HUB_DATABASE_URL=postgres://nama:nama@localhost:5432/nama_hub
export NAMA_HUB_SIGNING_KEYS="$(openssl genpkey -algorithm ed25519)"
SQLX_OFFLINE=true cargo run -p nama-hub
```

`SQLX_OFFLINE=true` has sqlx's macros check queries against `.sqlx/`: otherwise `sqlx.toml` has them check against `NAMA_HUB_DATABASE_URL`'s database, which the Hub migrates only once it is built.

A Server Links to it itself once its `hub_url` config, or `NAMA_HUB_URL`, names it, such as `NAMA_HUB_URL=http://localhost:7981 cargo run -p nama-server`. Linking needs an Account whose email a Sign-in Link proved (ADR 0100). Without Resend, start the Hub with `NAMA_HUB_LOG_MAIL=true` and `NAMA_HUB_PUBLIC_URL` set to where its pages are, `http://localhost:7981` with the `web` feature or `vp -C hub dev`'s address without it: the sign-in page then offers Sign-in Links, and asking for one writes the link to the Hub's log, to open from there.

## Importing Wikidata

The Hub knows films and series, which TMDB, IMDb and TVDB ids name each, and their cast, directors, writers, genres, subjects, awards, franchises and countries, from Wikidata's JSON dump, which Wikidata publishes weekly (ADR 0095). `nama-hub import-wikidata` reads the decompressed dump on stdin and replaces the titles with those in it, in one transaction: the Hub keeps answering from the last import meanwhile, and a dump cut short, which ends before its closing `]`, changes nothing. `nama-hub train-vectors` then builds each title's vector from those facts and from the pooled counts of Movies Accounts finished together, which outweigh the facts as more Accounts finish a title; a Movie Wikidata lacks gets one from those counts alone. Those vectors are how the Hub's Similar titles find the titles most alike, and its recommendations the titles nearest those an Account finished. It replaces the last run's vectors in one transaction too, so run it after each import. Run both weekly from the host's scheduler, as their own processes, never inside the serving one (ADR 0096), with the same `NAMA_HUB_DATABASE_URL`, after the serving Hub has migrated the database:

```sh
curl -fsS https://dumps.wikimedia.org/wikidatawiki/entities/latest-all.json.gz \
  | gzip -d | nama-hub import-wikidata && nama-hub train-vectors
# or, from the Docker image:
curl -fsS https://dumps.wikimedia.org/wikidatawiki/entities/latest-all.json.gz \
  | gzip -d | docker run --rm -i -e NAMA_HUB_DATABASE_URL ghcr.io/electather/nama-hub import-wikidata \
  && docker run --rm -e NAMA_HUB_DATABASE_URL ghcr.io/electather/nama-hub train-vectors
```

The dump is over 100 GiB compressed, so an import takes hours; it streams, holding nothing on disk. Its transaction holds the titles' tables all that time, so deploy a Hub whose migration changes them between imports: its startup would wait for the import to finish.

## Tests

The tests need that PostgreSQL. `#[sqlx::test]` reads `DATABASE_URL`, and makes a database of its own for each test. `SQLX_OFFLINE=true` builds them from `.sqlx/`, as CI does by leaving `NAMA_HUB_DATABASE_URL` unset: with it exported as above, the macros would check against that database, which may not be migrated yet:

```sh
SQLX_OFFLINE=true DATABASE_URL=$NAMA_HUB_DATABASE_URL cargo test -p nama-hub
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
