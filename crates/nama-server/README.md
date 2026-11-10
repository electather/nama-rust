# nama-server

The Nama Server: owns Libraries, streams media and handles Requests. Its database is SQLite (ADR 0005).

## Writing to the database

Every write goes through `Database::write`, which hands it to the single writer task that owns the only writable connection. Never open another writable connection. Long jobs such as a scan write in small batches so other writes and reads aren't held up. `src/db.rs` explains why.

## Behind a reverse proxy

A Server believes `X-Forwarded-For` and `X-Forwarded-Proto` only from the peers listed in `trusted_proxies` in `nama.toml` (ADR 0018). Each entry is an IP address or a CIDR range. The list is empty by default, so the headers are ignored: sign-in limits count each socket peer, and the session cookie is never `Secure`.

```toml
trusted_proxies = ["127.0.0.1", "10.0.0.0/8"]
```

From a trusted peer, the client is the rightmost `X-Forwarded-For` address that isn't itself a trusted proxy, and the cookie is `Secure` when the last `X-Forwarded-Proto` value is `https`. List every proxy between the client and the Server, and nothing else: any listed peer can name its own client address, which dodges sign-in limits, and claim HTTPS.

## ffmpeg

A Server Remuxes and Transcodes with the system's ffmpeg and ffprobe, 4.4 or later (ADR 0026). It runs the first of each on `PATH`, or the ones `nama.toml` names, which `NAMA_FFMPEG` and `NAMA_FFPROBE` override:

```toml
ffmpeg = "/usr/lib/jellyfin-ffmpeg/ffmpeg"
ffprobe = "/usr/lib/jellyfin-ffmpeg/ffprobe"
```

Without a usable pair it only Direct Plays, and says why in its startup log and on the Playback page. With one, a Version whose Tracks the Device decodes but whose container it can't is Remuxed over HLS, one whose video it can't decode is Transcoded to H.264 and AAC with libx264, tone-mapping HDR the Device can't show to SDR with zscale if ffmpeg has it, and ffmpeg writes their segments under `transcodes/` in the data directory. It runs at most 2 Transcodes at once, which an admin changes on the Playback page; while that many run, a Device that can play a Remux gets one, and one that can't is refused as busy. With ffprobe, each scan is followed by probing, in the background, the files not probed yet, or changed or read by an older probe version since, for their Tracks.

Tests make the media they probe with ffmpeg rather than commit sample files, so `cargo test` needs ffmpeg, built with libx264, libx265 and zscale (libzimg), and ffprobe on `PATH`.

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

Commit the changes under `.sqlx/`. CI's `rust-check` job runs `cargo sqlx prepare --check` and fails if they're stale. When you upgrade the `sqlx` crate, bump the `sqlx-cli` version here and in `.github/workflows/ci.yml` to the same minor version.

## Serving the web app

The `web` feature embeds the web app's build output (`web/dist/client`) and serves it (`src/web.rs`). It's off by default, so building and testing the Server needs no Node. Release builds turn it on in `dist-workspace.toml`. To work with it, build the web app first (see `web/README.md`):

```sh
vp -C web build
cargo run -p nama-server --features web
cargo test -p nama-server --features web --test web   # the tests that need it
```
