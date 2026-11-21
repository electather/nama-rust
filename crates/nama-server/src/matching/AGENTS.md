## Matching

Linking the Items a scan finds to titles from the Library's title sources, storing their metadata and ids on the Items, and fixing Matches by hand.

- **Decides under the scan lock.** Matching decides every Item still undecided after each scan, and fixes Matches by hand (`Matcher::set_match`, `clear_match`), under the Library's scan lock, so a fix waits for a running scan.
- **Stores only through `Contents`.** Matches, pins and Unmatched decisions are read and written through `library::Contents`; Matching holds no SQL.
- **Asks title sources only through `TitleSources`** (`library::TitleSources`), and reads its `Unanswered`.
- **Recognisers only propose; the Kernel decides** (ADR 0050). `recognisers.rs` asks them through `library::Recognisers` after the search decides, and Matching fetches what one names from the title sources as it does an ID Hint's, then stores the Match or Unmatched decision itself. A failing Recogniser leaves only its own Item undecided, never Unmatched (`Miss::RecogniserFailed`); unlike a failing title source, it doesn't stop the Library's other titles or the refresh.
- **Unit tests.** The area's unit tests share the fixture in `testing.rs` in this folder.
- **Metadata version.** A change to what a Match stores from the title sources bumps `METADATA_VERSION` in `../library/versions.rs`, so the next scan fetches every Matched title again by the ids its Match has and stores the new fields (ADR 0040).

### Neighbours

- Uses: `db`, `events`, `library` (`Contents`, `Libraries`, `TitleSources`, `Recognisers`, `SimilarSources`, `Artwork`, `ScanLocks`, `country_tag`) and `plugins` (`ExtensionPoints`).
- Used by: `scanner` (`Matcher`, `Summary`, `Error`, `count`) and the `api` part for matches (`Matcher`, `ManualError`). The Name parser version rule, which covers how Matching decides, lives in `scanner`'s `AGENTS.md`.
