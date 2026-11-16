# The Server's areas call down fixed rows

The Server stays one crate, regrouped into areas, one folder each, and a few single files. Who may call whom is a fixed order of rows, highest first:

1. `api`, `automation`
2. `browse`, `scanner`, `plugin_settings`
3. `requests`, `playback`, `matching`
4. `library`
5. `events`, `connections`, `users`
6. `plugins`
7. `images`, `server_settings`, `ffmpeg`, `forwarded`, `web`, `db`, `config`

A module may call a lower row, or a module to its right in its own row. `lib` and `main` start the Server and call any row. The table lives once, as `ROWS` in `crates/nama-server/tests/structure.rs`, and a test there fails on a file directly under `src/` that has no place in it.

Ten top-level files had no row. Each goes where its imports put it, so that no upward call results:

- **Into `playback/`:** `tracks`, `track_preferences` and `probing`, with `probing/`. Only playback, browse, scanner and api use them, all at or above playback's row, and probing needs `playback::subtitles`, which would be an upward call from any lower row. Playback's `AGENTS.md` takes their "One owner" rules.
- **Into `users/`:** `codes`, `device_names`, `rate_limits`, `sessions`, `sign_in` and `sign_in_codes`. `rate_limits` and `users` import each other, `sign_in` needs `codes`, `device_names`, `rate_limits` and `sign_in_codes`, and `sessions` needs `users`, so they form one area. `connections`, left of `users` in its row, uses `codes`, which stays allowed.
- **Single files in the bottom row:** `forwarded` reads only `config` and is used by api and sessions; `web` imports nothing from the crate.

`plugin_settings` stays a single file in the second row, as the design says: its folder holds only private helpers (`stored.rs`, `put_back.rs`, `running.rs`), and its one rule, "One owner for Plugin changes", stays in `crates/nama-server/AGENTS.md`, so it gets no `AGENTS.md` of its own.

Two calls are allowed against the order:

- **ADR 0012's upward call.** The Library makes Requests Available inside its own write, so a scan's Match and the Request's state change land in one transaction. The allowance is keyed on one file, `src/library/contents/decisions.rs`, and two functions: it imports `requests::make_available` and `requests::make_seasons_available` by path, so any other call into `requests` from that file still fails.
- **Tests may build a Scanner.** Unit tests (files named `*_tests.rs` or `testing.rs`) may build `scanner::Scanner`, since only `Scanner::new` makes `ScanLocks`, and a Matching or Requests test that needs a scan or a Matcher can't get one from a lower row.

The other upward calls were cut, each with no behaviour change: `events` calling `requests` and `playback::visible_to`, `server_settings` using `playback::PlaybackLimit`, and the test-only `plugins/wasm/title_source_tests.rs` calling `library`. `tests/structure.rs` reads each source file's `crate::` paths, skipping comments and strings, and fails, naming the file, line and import, on every call the order forbids that its `ALLOWED_UPWARD` table, holding the two calls above, doesn't list. It reads a macro's `$crate::` paths the same way, prints with each failure the reason of every entry covering that file and area, and fails on an entry no import uses, so an allowance goes once its call does.

## Considered Options

- **Component crates per area**, joined by events: rejected in the plugin-lanes design. Features share SQLite transactions, and an event can't replace a shared write.
- **Leave the ten files at the top level, each with its own row**: rejected. `rate_limits` and `users` import each other, so no row order separates them, and `probing` needs `playback::subtitles` from below.
- **Rules in prose only**: rejected. Nothing would notice a new upward call until a reviewer did; a test that reads the imports holds the order in place.

## Consequences

- A new top-level file fails `tests/structure.rs` until it gets a row in `ROWS`.
- A call the order forbids is cut with a value passed in, a trait the lower row defines, or the item moved down, rather than allowed in the table.
- Moving a module to another row is a decision recorded here, since it changes what every module above and below may call.
