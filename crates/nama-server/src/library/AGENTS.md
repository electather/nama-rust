## Library

What a Library holds: its contents, its settings (`libraries.rs`), asking its title sources, artwork, Custom Images and Watch State. Tracks join the area in a later stage.

- **Parts.** Each part is a `mod` of `library.rs` that neighbours address by name, such as `library::title_source::TitleSources`; its own files are private modules. `artwork` is public only so the integration tests reach it as `nama_server::artwork`.
- **One owner for a Library's contents.** Only `contents.rs` and its `contents/` folder read or write `library_files`, `library_items`, `item_matches`, `item_match_ids`, `match_pins` and `title_search`, tests included. Everything else goes through `Contents`. `OWNED_TABLES` in `crates/nama-server/tests/structure.rs` enforces it.
- **Matches make Requests Available in the same write.** A write that stores Matches makes the Matched Movies' and Seasons' Requests Available by calling `requests::make_available` and `make_seasons_available` (ADR 0012). It is the one accepted call up the rows.
- **Private modules.** Contents' files are private modules; neighbours import only what `contents.rs` re-exports, and items inside default to `pub(super)`. The one exception is `pub(crate) mod fixture`, which other areas' tests import as `library::contents::fixture`. The title id code is in `contents/ids.rs`.
- **A title's preferred id follows its Library's order.** Contents reads name a Movie or Show by `primary_id` in the order of the Library's `title_sources`, read in the same query; only what belongs to no Library, such as a Request, uses `DEFAULT_ORDER`.
- **Test helpers.** `contents/fixture.rs` is the one in-file fixture other areas' tests build on; `contents/testing.rs` holds what tests read back.
- **One owner for asking the title sources.** Only `title_source.rs` (`TitleSources`) calls a `TitleSource` and decides what its failures mean. Matching, Requests, Artwork and handlers ask it and read its `Unanswered`.
- **Artwork picks from every list.** Artwork picks from every ordered source's artwork list (ADR 0045, written later this stage), by language, then source order, then rating.
- **One owner for Custom Images.** Only `custom_images.rs` reads or writes `custom_images`, tests included, and keys a title's by its external id, never an Item or file, and a Plugin's by the Plugin's id (ADR 0036). A Library's goes with the Library. Browse, Requests, Libraries and Plugins ask it for the URLs to show in place of downloaded images and Manifests' logos.
- **One owner for Watch State.** Only `watch_states.rs` reads or writes `watch_states`, tests included, and keys it by a Movie's or Episode's external id, never an Item or file (ADR 0013). Browse works out which Movie or Episodes a Version or a marked title stands for and asks `WatchStates`.

### Neighbours

- Uses: `db`, `events`, `requests` (the ADR 0012 call), `plugins` (`ExtensionPoints`), `images`, `server_settings` (`ImageSettings`), and, until a later stage cuts them, `matching::METADATA_VERSION` and `scanner::PARSER_VERSION` up the rows.
- Used by: `scanner`, `matching`, `browse`, `playback`, `probing`, `tracks`, `track_preferences`, `requests`, the `api` parts and startup in `lib.rs`.
