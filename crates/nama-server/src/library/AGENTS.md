## Library

What a Library holds. Contents is its first part; the other Library files (libraries, watch states, custom images, tracks) join the area in later stages.

- **One owner for a Library's contents.** Only `contents.rs` and its `contents/` folder read or write `library_files`, `library_items`, `item_matches`, `match_pins` and `title_search`, tests included. Everything else goes through `Contents`. `OWNED_TABLES` in `crates/nama-server/tests/structure.rs` enforces it.
- **Matches make Requests Available in the same write.** A write that stores Matches makes the Matched Movies' and Seasons' Requests Available by calling `requests::make_available` and `make_seasons_available` (ADR 0012). It is the one accepted call up the rows.
- **Private modules.** Contents' files are private modules; neighbours import only what `contents.rs` re-exports, and items inside default to `pub(super)`. The one exception is `pub(crate) mod fixture`, which other areas' tests import as `library::contents::fixture`. The title id code is in `contents/ids.rs`.
- **Test helpers.** `contents/fixture.rs` is the one in-file fixture other areas' tests build on; `contents/testing.rs` holds what tests read back.

### Neighbours

- Uses: `db`, `events` (`event`), `requests` (the ADR 0012 call), and, until a later stage cuts them, `matching::METADATA_VERSION` and `scanner::PARSER_VERSION` up the rows. Its tests use `libraries`.
- Used by: `scanner`, `matching`, `browse`, `playback`, `probing`, `tracks`, `track_preferences`, `artwork`, `custom_images`, `metadata`, `requests`, startup in `lib.rs`, and the `api` parts for Libraries, browsing, Matches and Requests.
