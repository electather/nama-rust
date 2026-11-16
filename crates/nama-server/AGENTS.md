## nama-server

### Data

- **One database writer.** Every write goes through `Database::write` or `Database::write_then` in `src/db.rs`, on the one writable SQLite connection. Long jobs such as a scan write in small batches.
- **One owner for Tracks.** Only `src/tracks.rs` reads or writes `version_probes`, `tracks` and `probe_failures`, tests included. Browse attaches them to the Versions `Contents` reads.
- **One owner for probing.** Only Probing (`src/probing.rs`, with its private `src/probing/ffprobe.rs`) runs ffprobe on stored files and decides what a probe failure means for trying it again (`ProbeFailure`); Tracks stores it.
- **One owner for Server settings.** Only `src/server_settings.rs` reads or writes `server_settings`, tests included.
- **One owner for Track Preferences.** Only `src/track_preferences.rs` reads or writes `track_preferences`, tests included, and decides what a Preference is kept for: the Show or Movie a Version is of, never an Item or file (ADR 0035).
- **One owner for Plugin changes.** Outside tests, only `src/plugin_settings.rs` enables or disables a Plugin or saves its settings, under its one lock and in ADR 0028's order. The type enforces it: only Plugin settings and startup hold the `Registry`, and every other module holds its `ExtensionPoints`, which calls Plugins but can't enable, disable or list them. Handlers and tasks ask `PluginSettings`.
- **Migration list.** A new migration's file name also goes as the last line of `migrations.txt`, so two PRs adding the same migration number conflict in git (#561); a test checks it.
- **Query data.** After changing a `sqlx::query!` or anything in `migrations/`, regenerate `.sqlx/` at the workspace root and commit it (commands in `README.md`).
- **Probe version.** A change to what probing reads from a file, or how, bumps `PROBE_VERSION` in `src/tracks.rs`, so the next pass after a scan probes every file again (ADR 0023's pattern).
- **Probe failure version.** A change that may let ffprobe read files it lastingly failed on before, without changing what probing reads from a file that it could read, bumps `PROBE_FAILURE_VERSION` in `src/tracks.rs`, so the next pass tries again only those files.
- **WebVTT version.** A change to the WebVTT a subtitle source makes bumps `WEBVTT_VERSION` in `src/playback/subtitles.rs`, so what the cache kept from before is made again.
- **Whole-Library reads.** Browse reads a Library's whole contents even to find one title. Each such read is marked `// ponytail: whole-Library read`; query by id there once Libraries get big.

### Security

- **Secrets stay out of storage and logs.** The database holds the Authenticator's credential string and the SHA-256 hash of each session token; only `src/users.rs` derives them. Types holding a token or Sign-in Code skip `Debug`. The Setup Code is the one secret logged, on purpose. Plugin settings are the one secret stored as written, since a Plugin must send a key such as TMDB's as it is: no setting value is ever logged (the `Debug` of `SettingValue` and `PluginTables` hides them), and a secret's value, stored or default, is never sent to a client (`setting_to_proto` in `src/api/plugins.rs`).
- **Clients name ids.** Media is served by Version id (`Contents::version_file`) and artwork only for paths some Match stores (`Contents::stores_image`) or a Movie or Show search or lookup handed out (`Artwork::offer`). Where a Library lives on disk is shown only to Users with `MANAGE_LIBRARIES`, except that a User with `DELETE_MEDIA` is told the read-only folder that refuses a delete.
- **One owner for who sees an Event.** Only `src/events.rs` decides which subscriber gets an Event and what of it; handlers pass the caller.
- **Guessable requests go through `SignIn` (`src/sign_in.rs`).** It lets an attempt through before reading a User or a secret, and un-counts it the moment the secret checks out; a failure after that, or an input check before it, is no guess. Sign-in Codes count only an unknown or expired code.
- **Forwarded headers from trusted proxies only.** Read where a request came from (client IP, HTTPS) from the `RequestSource` that `src/forwarded.rs` sets. It believes `X-Forwarded-*` only from a peer in `Config::trusted_proxies` (ADR 0018), since any client can set them.

### Playback

- **What a plan converts is worked out once.** Before the steps run, `candidates` in `src/playback.rs` works out what a Remux and a Transcode of the Version would convert, and the plan keeps the one it narrowed to in `Playing`, with the Version's length, which a Remux's or a Transcode's playlist lists, and, for a Transcode on hardware, what it falls back to in software and why. Its reasons, Tracks, events, playlist and ffmpeg arguments read that value; none asks the planning facts again what it converts, how long the Version is or what it falls back to.

### Tests

- **One fixture.** Server tests in `tests/` build on `TestServer` in `tests/support`, started through `TestServer::builder()`. Their setup goes through the API, as a client's would; a shared helper never writes the database. A test file adds a fixture of its own only for what that file alone needs, and a shortcut one test needs stays in that test's file.
- **One in-file fixture.** An in-file test that needs a Library with a Matched Movie builds it with `library::contents::fixture` and adds what else it needs on top, so a change to how a Match is stored touches one place.
- **Fake programs.** Fake programs a test runs are written only through `ffmpeg::fake::script`. Writing one with `fs::write` can fail with "Text file busy" (#411).
- **Unit tests sit beside their file.** `foo.rs`'s unit tests live in `foo_tests.rs` beside it, which `foo.rs` declares as `#[cfg(test)] #[path = "foo_tests.rs"] mod tests;`, so each test keeps its name. A file not moved yet keeps its `mod tests` inline until its area is regrouped.
- **One clock.** Server code reads the time only through `db::now` or `db::now_ms`, which a Plugin's `nama.now` reads too, so a test moves every clock the Kernel or a Plugin sees with `db::advance_clock` (`testing` feature, process-wide; `TestServer::advance_clock` in `tests/support`).

### Structure

- **File size.** No `.rs` file under `src/` or `tests/`, nor `build.rs`, is over 800 lines, except those `tests/structure.rs` lists with a ceiling, a multiple of 100 lines the file stays at or under. A ceiling only comes down, to the next 100 above the file as it shrinks, and its entry goes once the file is 800 lines or fewer. The table takes a new file only when a move-only change makes it out of a listed one.
- **Owned tables.** Each table a "One owner" rule here or in an area's `AGENTS.md` names is listed with its owner in `tests/structure.rs`, which fails on SQL naming it in any other file, tests included. A new such rule adds its tables there.
- **Named paths exist.** A path an `AGENTS.md` here names in backticks, one with a `/` in it, exists relative to that file's folder or the workspace root. `tests/structure.rs` checks it, so a move updates the rules that name what moved.
- **Rows.** A module calls only a lower row, or a module to its right in its own row, of `ROWS` in `tests/structure.rs`; a new top-level file gets a place there (ADR 0051).
- **Area rules.** An area's folder holds an `AGENTS.md` with the area's own rules and its neighbours; read it before changing the area. A rule lives once, in the nearest `AGENTS.md`.
