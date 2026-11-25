# Nama

A self-hosted media server and request manager, with an optional central Hub for identity and Server discovery. [`GLOSSARY.md`](GLOSSARY.md) defines the terms used throughout, and [`docs/adr/`](docs/adr) records the decisions behind the design.

## Layout

| Path | What it is |
|---|---|
| [`crates/nama-server`](crates/nama-server) | The Server: owns Libraries, streams media and handles Requests |
| [`crates/nama-hub`](crates/nama-hub) | The Hub: Accounts and Server discovery |
| [`crates/nama-hub-eval`](crates/nama-hub-eval) | Scores the Hub's recommendations against MovieLens ratings, in development only (ADR 0095) |
| [`crates/nama-plugin-api`](crates/nama-plugin-api) | Extension Point traits through which Plugins take part in a Server |
| [`crates/nama-pdk`](crates/nama-pdk) | Plugin development kit for Installed Plugins |
| [`crates/nama-proto`](crates/nama-proto) | Rust types and ConnectRPC stubs generated from the client API in `proto/nama/v1/` |
| [`crates/nama-plugin-proto`](crates/nama-plugin-proto) | Rust types generated from the Plugin contract in `proto/nama/plugin/`, and its version |
| [`crates/nama-forwarded`](crates/nama-forwarded) | Where a request to the Server or the Hub came from, believing forwarded headers only from trusted proxies (ADR 0018) |
| [`crates/nama-hub-proto`](crates/nama-hub-proto) | Rust types and ConnectRPC stubs generated from the Hub API in `proto/nama/hub/` |
| [`crates/nama-test-plugin`](crates/nama-test-plugin) | An Installed Plugin used only in the Server's tests |
| [`crates/nama-structure`](crates/nama-structure) | The structure checks the Server's and the Hub's tests run on their own files (ADR 0099) |
| [`crates/nama-test-report`](crates/nama-test-report) | CI tooling: records test results and reports flaky and slow tests |
| [`plugins/`](plugins) | Built-in Plugins, the Installed Plugins the Server bundles, and the libraries they share |
| [`web/`](web) | The web app the Server serves, the Hub's web app, and nama-ui, the design system they share (ADR 0101) |
| [`docker/`](docker) | The Server's Docker image |
| [`proto/`](proto) | The Protobuf API schema |

[`CONTRIBUTING.md`](CONTRIBUTING.md) says where the rules live and how a change lands.

## Metadata from TMDB

A Server Matches the files in its Libraries to Movies and Shows using the built-in TMDB Plugin ([`plugins/tmdb`](plugins/tmdb)). It needs a TMDB API key, which you can get for free from [themoviedb.org](https://www.themoviedb.org/settings/api). Without a key the Plugin isn't enabled and every file stays Unmatched. An Admin sets the key on TMDB's page under Plugins in the web app, and it takes effect without a restart.

Plugin settings live in the Server's database. On its first start, fresh or after upgrading, a Server imports any `[plugins.*]` tables from an existing `nama.toml` once, such as:

```toml
#:schema https://raw.githubusercontent.com/electather/nama-rust/main/docs/nama.schema.json

[plugins.tmdb]
api_key = "your TMDB API key"
# language = "en-US"   # the language titles, overviews and Title Logos are fetched in; its country picks the Content Rating
```

After that it ignores those tables, and logs that it does, so editing them changes nothing. The `#:schema` line lets an editor check the file ([`crates/nama-server/README.md`](crates/nama-server/README.md), "Editing `nama.toml`").

TMDB also says which Box Set a Movie belongs to, such as the Alien films. A Movie's page shows the rest of its Box Set, and each Movies Library has a Box Sets tab. Its Movies the Server lacks are listed too, for Users who may Request Movies to Request, unless an Admin turns off TMDB's "List Box Set Movies the Server lacks" setting, which saves about one TMDB request per Movie a Box Set lists each time its Movies are fetched.

A file or folder name can name the Edition its Version holds, as Plex writes it: `Alien (1979) {edition-Director's Cut}.mkv`. The web app shows "Director's Cut" beside that Version. A folder's Edition applies to every file in it that names none of its own, and an Edition never changes what a file Matches.

The Server also reads the IDs, and nothing else, from the `.nfo` files Kodi, Jellyfin and Emby leave beside videos: a Movie's `<video name>.nfo`, a stacked Movie's shared one, or `movie.nfo` in a folder holding one Movie, a Show's `tvshow.nfo`, and an Episode's own `<video name>.nfo`. A file Matches the title its `.nfo` file names, without a search. IDs in the file's name count only when the title sources have none of the `.nfo` file's. Adding, editing or deleting an `.nfo` file Matches its video again on the next scan, unless an Admin set its Match by hand. The Server skips an `.nfo` file over 4 MiB, a link, or one naming no ID it can read, and logs a warning naming it.

## Requests to Radarr and Sonarr

Approved Movie Requests go to Radarr through the Radarr Plugin ([`plugins/radarr`](plugins/radarr)), and Approved Season Requests to Sonarr through the Sonarr Plugin ([`plugins/sonarr`](plugins/sonarr)), both Installed Plugins bundled in the Server. An Admin enables each on its page under Plugins with its URL and API key, and "Test" offers its quality profiles and root folders to pick from. Sonarr finds a Show by its TVDB id, so a Show the metadata source has none for can't go to Sonarr. The TVDB Plugin ([`plugins/tvdb`](plugins/tvdb)), also bundled, serves Show metadata from TVDB and needs a TVDB API key. The fanart.tv Plugin ([`plugins/fanart`](plugins/fanart)), also bundled, serves logos, backdrops, posters and other artwork from fanart.tv for Movies and Shows, and needs a fanart.tv API key. The ntfy Plugin ([`plugins/ntfy`](plugins/ntfy)), also bundled, tells each User about their Requests on the ntfy topic they set in their Personal settings, on ntfy.sh or an ntfy server the Admin names. The Discord Plugin ([`plugins/discord`](plugins/discord)), also bundled, tells each User about their Requests through the Discord webhook they set in their Personal settings. The Webhook Plugin ([`plugins/webhook`](plugins/webhook)), also bundled, POSTs every Request event as JSON to a URL the Admin sets, signed in an `X-Nama-Signature` header with a key they may set. The MDBList Plugin ([`plugins/mdblist`](plugins/mdblist)), also bundled, shows up to 3 public MDBList lists an Admin names as rows on every User's home page, and needs an MDBList API key. The OpenSubtitles.com Plugin ([`plugins/opensubtitles`](plugins/opensubtitles)), also bundled, finds and downloads subtitles from OpenSubtitles.com for Users with Download subtitles, and needs an API key from an OpenSubtitles.com account's API consumers and that account's username and password, against whose daily download quota every download counts; "Test" logs in with them. Granting it the Store lets it keep its login between downloads, since OpenSubtitles limits logins more tightly than other calls; without it, each download logs in. The Top Picks Plugin ([`plugins/top-picks`](plugins/top-picks)), also bundled, gives each User a "Top picks" row on their home page of the titles they haven't started, ranked by the genres they play most and their Watchlist, and a "Family night" row of those whose Content Rating is one the Admin names, G to PG-13 and TV-Y to TV-PG unless they change it. It also suggests the best few for the top of home. The Discover Plugin ([`plugins/discover`](plugins/discover)), also bundled, shows rows on every User's home page, those an Admin picks among its Shared rows, at most 20: from TMDB, such as Trending this week, Top rated movies, Now in theaters for the Server's Region and Hidden gems, and Best Picture winners, Oscar nominees, Palme d'Or winners and Golden Globe best dramas from Wikidata, asked about monthly and kept in its Store if the Admin grants it. It needs a TMDB API key, which "Test" checks. Among its Your rows, also at most 20, it gives each User up to three Because you watched rows of TMDB's recommendations for the titles they played most recently, through or past halfway, up to two More rows of TMDB's top titles in the genres they play most, a Your Watchlist row of the Watchlist titles they haven't started, newest added first, and a Watch it again row of the titles on the Server they finished over a year ago, those of the genre they play most first. Because you watched and More rows leave out what the User started. A Starring row and a From row follow the Person among a title's first 5 Cast, and the Director or Creator, in the most titles the User played, and leave out what they played; granting it the Store keeps each title's Roles there, so a refresh asks TMDB only for titles played since. The Genres and People Plugin ([`plugins/genres-people`](plugins/genres-people)), also bundled, adds to a title's Similar titles those in the Libraries that share its director, creator, writer, main cast or most of its genres, once an Admin enables it granting Read Libraries; a title Matched after another shows in that one's row from Upkeep's next weekly metadata refresh. The S3 Plugin ([`plugins/s3`](plugins/s3)), also bundled, keeps the Server's stored files, such as its images' originals, in an S3 or S3-compatible bucket once an Admin picks it as the Server's blob store. The TheIntroDB Plugin ([`plugins/theintrodb`](plugins/theintrodb)), also bundled, asks TheIntroDB where a Movie's or Episode's Intro, Recap and Credits are, for each Library that lists it among its Segment sources. Without its optional API key it keeps to 30 requests every 10 seconds, counting them in its Store if the Admin grants it. The Trakt Plugin ([`plugins/trakt`](plugins/trakt)), also bundled, adds what each User plays to the history of the Trakt account they link in their Personal settings, removes what they mark unplayed, and every 15 minutes brings back what they watched elsewhere, for titles with a TMDB or TVDB id. It needs the Client ID and Client Secret of a Trakt API app, and the Store, which keeps each User's link working as Trakt renews it. Asking for a whole Show also has Sonarr monitor the Show's new Seasons, until its Followed Show ends. Deleting a Movie, or one of its Versions, first has Radarr stop monitoring it, so it doesn't download it again; Radarr keeps the Movie. Deleting an Episode, or one of its Versions, has Sonarr stop monitoring that Episode, and deleting a Season that Season and its Episodes. Deleting a whole Show has Sonarr stop monitoring every Season it has now, but Sonarr keeps the Show and still monitors its new Seasons if it did, so a Followed Show keeps getting them. If Radarr or Sonarr can't be told, the delete still happens, and the web app says it may download what was deleted again.

## Custom Images

A User with Manage Libraries can upload their own Poster or Backdrop for a Movie or Show, or their own Poster for a Season, with "Edit images" on its page. There they can also pick a Movie's or Show's Poster or Backdrop, or a Season's Poster, from the others its title sources have, which the Server downloads and keeps as if it had been uploaded. They can also upload a Backdrop for a Library, on its page in Settings' Libraries section, which the home page then shows the Library by in place of its newest title's. The Server keeps them in `custom-images/` in its data directory (`/data/custom-images` in Docker). They exist nowhere else, so back them up together with the database, `nama.db`, which records which image is current: `nama-server backup <file>.tar` writes both, with `nama.toml`, into one file, safely while the Server runs, and `nama-server restore <file>.tar`, run with the Server stopped, puts them back, keeping each file it replaces beside it with `.before-restore` added to its name (ADR 0068). The Server also writes one itself every day at 03:00, into `backups/` in its data directory, and keeps the newest 7 it wrote; in Settings' Backups section, an Admin picks another folder, such as a mounted share for copies off the machine, and how many to keep, and backs up now. Upkeep's Back up Job, in Settings' Jobs section, sets when, or turns it off. A backup holds Plugin settings, API keys included, so keep it as safe as the data directory. In Docker, run them with `docker exec nama nama-server backup /data/nama.tar`, and restore with `docker run --rm -v nama-data:/data ghcr.io/electather/nama-server restore /data/nama.tar` while the container is stopped. Before an upgrade changes the database, the Server copies it to `nama.db.before-upgrade` beside it, replacing the copy the last upgrade left; if it can't, it doesn't upgrade or start, and says how much free space the copy needs. To go back to the version you upgraded from, stop the Server, delete `nama.db-wal` and `nama.db-shm` if they're there, rename `nama.db.before-upgrade` to `nama.db`, and start that version. The other images in the data directory are downloaded or written again when they're missing.

A `poster.jpg` or `fanart.jpg` that Jellyfin, Kodi and the like leave beside a Movie's file, or one named after the file such as `Alien (1979)-poster.jpg`, shows in place of the downloaded Poster or Backdrop after the next scan, unless the Movie has a Custom Image. A name alone like `poster.jpg` counts only in a Movie's own folder, not in the Library's top folder, where it would fit every Movie. It must be a JPEG, PNG or WebP a Custom Image could be; the scan logs one it skips. The Server reads these files and never changes them. When a Movie's Items in different folders each have one, the one beside the Item added first shows.

## Docker

Each release, prereleases aside, is also a Docker image for amd64 and arm64, `ghcr.io/electather/nama-server`, with [jellyfin-ffmpeg](https://github.com/jellyfin/jellyfin-ffmpeg) as its ffmpeg and ffprobe, the easiest way to get hardware Transcoding. It runs as user 1000, keeps its state in `/data`, and reads `/data/nama.toml` if there is one. The image sets `data_dir`, `ffmpeg` and `ffprobe` through `NAMA_*` variables, which beat that file, so change those with `-e`. Mount each Library's folder and give the Library the path inside the container:

```sh
docker run -d --name nama -p 7980:7980 \
  -v nama-data:/data \
  -v /srv/movies:/media/movies:ro \
  ghcr.io/electather/nama-server
```

User 1000 must be able to read the media, and write `/data` if you mount a host folder there. The `:ro` mount above keeps the Server from deleting media: to let Users with the Delete media Permission delete it, mount the folder without `:ro`, give user 1000 write access to it, and scan the Library again. To run as someone else, pass `--user` with their ids.

The container's time zone, UTC unless you change it, is the one Plugins' Job schedules are read in, and the Server logs it at startup. Set it with `-e TZ=Europe/London`, or give it the host's with `-v /etc/localtime:/etc/localtime:ro`.

Keep `/data` on a local disk. On a network share, such as NFS or SMB, the database can lock up or corrupt, and the Server warns at startup. It also warns on a copy-on-write filesystem, ZFS or Btrfs, where the database can slow down, or lock up in setups such as ZFS inside an LXC container; if it does, move `/data` to a filesystem such as ext4 or XFS.

For Intel or AMD hardware acceleration (VAAPI and QSV), pass the GPU and the group that may use it:

```sh
  --device /dev/dri:/dev/dri --group-add "$(stat -c %g /dev/dri/renderD128)"
```

The Server tests each GPU it is given and prefers a dedicated one to one built into the CPU. An Admin can pick the GPU Transcodes run on instead, under Hardware acceleration in Settings' Server Playback, or keep the Server off a GPU altogether by passing only the other one's node, such as `--device /dev/dri/renderD129`.

For NVIDIA (NVENC), install the [NVIDIA Container Toolkit](https://docs.nvidia.com/datacenter/cloud-native/container-toolkit/latest/install-guide.html) on the host and add `--gpus all`.

An Admin sees which ffmpeg the Server found, and its version, in Settings' Server Playback.

## Diagnostics

A Server logs to stdout and sends nothing anywhere else unless its admin says so (ADR 0054). To see where its time goes, set `OTEL_EXPORTER_OTLP_ENDPOINT` to an OpenTelemetry Collector's OTLP/HTTP address, such as `http://otel-collector:4318`. The Server then also exports its logs, and a trace for each API call, Scan, probe, image resize and ffmpeg launch, with a span for each SQL query and Plugin call in it. Spans carry ids, counts and durations, never titles, file paths or names; the logs are those on stdout. The standard `OTEL_*` variables configure the rest, such as `OTEL_SERVICE_NAME` and `OTEL_TRACES_SAMPLER`. The Server exports over plain HTTP only, so an HTTPS backend needs a Collector in front of it. The web app then records its own spans too, for each API call, page change and playback start, and sends them through its Server, which forwards them to the same endpoint; with the variable unset, the web app doesn't even download the code that records them.

An admin can also share with Nama's developers, under Sharing in Settings' Server section, both off by default: Diagnostics, the Server's spans and the few log lines a reviewed list in `crates/nama-server/src/diagnostics.rs` names, and, separately, Usage Analytics, named events such as a Request made or playback started. They go to the Hub through the Hub Plugin while the Server is Linked, never carrying titles, file paths, names or addresses, and the Hub forwards them unchanged to the developers' backend. With neither on, nothing is sent.

### Finding a slow request

[`docker/otel-lgtm.yml`](docker/otel-lgtm.yml) runs [`grafana/otel-lgtm`](https://github.com/grafana/docker-otel-lgtm), a Collector, Tempo, Loki, Prometheus and Grafana in one container, to look at a Server's Diagnostics on your own machine. It works with `docker compose` and `podman compose` alike:

```sh
docker compose -f docker/otel-lgtm.yml up -d
```

Run the Server pointed at it, such as from a checkout:

```sh
OTEL_EXPORTER_OTLP_ENDPOINT=http://localhost:4318 OTEL_SERVICE_NAME=nama-server cargo run -p nama-server
```

Use the app until it's slow, then open Grafana at <http://localhost:3000>; it needs no sign-in. Under Explore, pick Tempo, switch to TraceQL and search for one ConnectRPC method, such as `{name="nama.v1.BrowseService/ListMovies"}`, then sort the results by Duration and open the slowest; `{span.rpc.service="nama.v1.BrowseService" && duration > 200ms}` finds every slow call to a service. A trace takes about 30 seconds to show up in a search. Under Explore with Loki, `{service_name="nama-server"}` shows the Server's logs; a line logged during a trace has a "Trace:" link among its fields that opens that trace. Everything is lost when the container is removed.

### Sending Diagnostics to Grafana Cloud

Grafana Cloud takes OTLP over HTTPS with a login, so a Collector sits between it and the Server. [`docker/otel-grafana-cloud.yml`](docker/otel-grafana-cloud.yml) runs one. Copy [`docker/.env.example`](docker/.env.example) to `docker/.env`, which git ignores, and fill in the OTLP endpoint and the `Authorization=Basic …` header from your stack's OpenTelemetry tile; the value is everything after `Authorization=`.

Start it, then point the Server at it, from the same host or from another container on the same compose network:

```sh
docker compose -f docker/otel-grafana-cloud.yml --env-file docker/.env up -d
OTEL_EXPORTER_OTLP_ENDPOINT=http://localhost:4318 OTEL_SERVICE_NAME=nama-server cargo run -p nama-server
```

Traces and logs show up under Explore in your stack's Grafana, with the same queries as above. The free plan keeps 14 days; set `OTEL_TRACES_SAMPLER` on the Server if you near its monthly allowance.

## Checks

CI ([`.github/workflows/ci.yml`](.github/workflows/ci.yml)) runs the following, and every warning fails it:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo clippy --workspace --locked -- -D warnings
cargo clippy $(.github/scripts/wasm-packages.sh) --target wasm32-unknown-unknown --locked -- -D warnings
cargo clippy -p nama-server --features web --all-targets --locked -- -D warnings   # after `vp -C web build`
cargo clippy -p nama-hub --features web --all-targets --locked -- -D warnings      # after `vp -C web/hub build`
cargo nextest run --workspace --locked
cargo test --workspace --doc --locked
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked
RUSTDOCFLAGS="-D warnings" cargo doc -p nama-server --no-deps --document-private-items --locked
cargo machete
cargo deny check bans licenses sources
```

Building the Server builds the Plugins it bundles for wasm32 (`crates/nama-server/build.rs`), and its tests build the test Plugin, so both need the `wasm32-unknown-unknown` target (`rustup target add wasm32-unknown-unknown`).

`cargo deny check advisories` fails a PR only on an advisory its own `Cargo.lock` adds, not one already on main ([`.github/scripts/advisories.sh`](.github/scripts/advisories.sh)). The [`advisories`](.github/workflows/advisories.yml) workflow checks main daily and opens a `needs-triage` issue for each advisory it finds there.

`buf breaking` fails a change that breaks the client API in `proto/nama/v1/` or the Hub API in `proto/nama/hub/v1/` against main, unless the change declares the break with a new entry in [`proto/BREAKING.md`](proto/BREAKING.md) ([`.github/scripts/buf-breaking.sh`](.github/scripts/buf-breaking.sh), ADR 0032). The Plugin contract in `proto/nama/plugin/` has its own version instead (ADR 0041).

CI also checks that the generated code and the sqlx query data are current. If you change `proto/`, see [`crates/nama-proto/README.md`](crates/nama-proto/README.md). If you change a query or a migration, see [`crates/nama-server/README.md`](crates/nama-server/README.md), or the Hub's [`crates/nama-hub/README.md`](crates/nama-hub/README.md).

Its jobs run side by side on GitHub-hosted runners: `lint`, `rust-check`, `test` (split in two, on Linux for a PR and also on macOS and Windows on main), the web app's checks in `web` ([`web/README.md`](web/README.md), "Checks"), `web-test`, which builds the web app, lints the Server with `web` on and runs `cargo nextest run -p nama-server --features web --test web` against it, then builds the Hub's, lints the Hub with `web` on and runs its `web::` unit tests, and `hub`, which runs the Hub's tests against PostgreSQL in a service container, which only Linux runners have, so `test` leaves the Hub out. On a PR, path filters skip the side it doesn't touch: a web-only PR skips the Rust checks and the workspace tests, a Rust-only one the web checks, and one that touches neither, such as docs or Markdown no check reads, runs nothing. A push to main runs everything. A PR merges into main only once `ci-ok` passes, which waits for all the others and passes when none failed.

### Flaky and slow tests

CI runs nextest with its `ci` profile ([`.config/nextest.toml`](.config/nextest.toml)), and vitest with `CI` set, so a failed test is retried twice there; locally it isn't. The [`test-report`](.github/workflows/test-report.yml) workflow then records every test's result on the ref `refs/ci/test-history`, summarises the run's flaky and slow tests, and opens or updates a `needs-triage` issue per flaky test (ADR 0022). It holds one file per day for the last 60 days, and isn't a branch so clones skip it (ADR 0073). To ask the history your own questions:

```sh
git fetch origin refs/ci/test-history:refs/ci/test-history
git show ci/test-history:2026-11-20.jsonl | jq -c 'select(.outcome != "pass")'
```

## License

The Server, Hub, `nama-plugin-api` and Built-in Plugins are AGPL-3.0-only ([`LICENSE`](LICENSE)). The contracts that others build against, `nama-proto`, `nama-plugin-proto`, `nama-hub-proto`, `nama-pdk` and `proto/`, are MIT OR Apache-2.0 (ADR 0009). Each crate's `Cargo.toml` states its license.

## Attribution

This product uses the TMDB API but is not endorsed or certified by TMDB. Metadata and images are provided by [The Movie Database (TMDB)](https://www.themoviedb.org).

The web app draws ASS subtitles with [JASSUB](https://github.com/ThaUnknown/jassub), which bundles libass, FreeType, HarfBuzz, FriBidi and more. Their notices ship with the web app, at `/licenses/jassub.txt` on a Server, and the SIL Open Font License of JASSUB's fallback font, Liberation Sans, at `/licenses/liberation-sans.txt`.
