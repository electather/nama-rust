## Scanner

Walking each Library's folders, storing the video files in them and what their names say they hold, the sidecar subtitles beside them, copies of the images beside them or in a Show's or Season's folder (`LocalImages`), watching the folders for changes, and deleting media.

- **One path for Library changes.** Outside tests, creating, updating or deleting a Library goes through `Scanner::create_library`, `update_library` or `delete_library`, which alone do what must follow it (re-watching folders, cleaning a deleted Library's cached subtitles, Custom Image and copies of the images beside its media) in a task that outlives the caller (ADR 0029). Handlers read through `Libraries`.
- **One path for deleting media.** Every delete of media files, the API's and later the cleanup Plugin's (#700), goes through `Scanner::delete_media`, or `Scanner::start_delete` for a Plugin, which starts it without waiting and logs its failure; either deletes under the Library's scan lock in a task that outlives the caller, ends the Playback Plans playing what goes first, removes the Versions through `Contents`, logs who deleted what and cancels the open Requests still waiting on it through `Requests::apply` (ADR 0038).
- **A scan writes in batches under the scan lock.** A scan holds the Library's scan lock (`library::ScanLocks`) from reading what is stored through Matching, and stores its changes in batches of `BATCH_SIZE`, so other writes don't wait long behind it.
- **Recognisers see what the name parser couldn't place.** `recognised.rs` asks the Library's enabled Recognisers, in the Library's order, about the files the name parser made no Item of (ADR 0050).
- **Name parser version.** A change to how names are parsed or how Matching decides that could turn an Unmatched Item into a Match bumps `PARSER_VERSION` in `../library/versions.rs`, so the next scan parses stored files again and retries their automatic Unmatched decisions (ADR 0023). So does a change to what Matching stores about an automatic Unmatched decision, such as why it was made, so decisions stored before it get it.
- **Unit tests.** The area's unit tests share the `Fixture` in `scanner_tests.rs` in this folder.

### Neighbours

- Uses: `db`, `events`, `library` (`Contents`, `Libraries`, `ScanLocks`, `PARSER_VERSION`, `Artwork`, `CustomImages`, `LocalImages`), `matching` (`Matcher`, `count`), `playback` (`Probing`, `RunningPlans`, `Sidecars` with `Sidecar`, `VersionSidecars` and `is_sidecar`, and `SubtitleCache` and `Tracks` in tests), `requests`, `plugins`, `server_settings` (`ServerUrl`), and `images`, `ffmpeg` and `log_capture` in tests.
- Used by: `api` (libraries), `automation`, `browse` (`Doomed`, `read_only_folder`), and startup in `lib.rs`.
