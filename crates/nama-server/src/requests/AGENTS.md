## Requests

Requests and Followed Shows: Users asking for what isn't on the Server (ADR 0012).

- **One path for Request changes.** Every change to a Request or Followed Show goes through `Requests::apply` in `apply.rs`, which alone publishes their events and hands off (ADR 0029). A Library's write making them Available is the exception (ADR 0012).
- **Requester rules once.** What a Request and a Followed Show share (Requesters, the guarded Pending decision) is written once in `asked.rs`; each keeps only what differs. Who may see one is `users`' (`User::whose_requests`, `may_see`), which `events` asks too.
- **One owner for Quality rules.** Only `qualities.rs` reads or writes `qualities` and `default_qualities`, tests included, and decides which Quality a Request, Show or Followed Show takes (ADR 0025). Requests and handlers ask it. `OWNED_TABLES` in `crates/nama-server/tests/structure.rs` enforces it.
- **One owner for title ids.** Only the Requests area reads or writes `request_title_ids` and `followed_show_ids`, tests included; `OWNED_TABLES` enforces it. A Request's and a Followed Show's Show ids come only from these tables or the title source.
- **Private modules.** The area's files are private modules; neighbours import only what `requests.rs` re-exports, `qualities`' names included under names that read without it (`list_qualities`, `QualityRefused`). Items inside default to `pub(super)`.
- **Shared SQL is a literal.** SQL that several queries share is a string literal a macro hands to each query (`state.rs`, `progress.rs`, `asked.rs`), since sqlx reads a query from literals alone. One saying whether a Request or Followed Show is open must match its index. `hand_off.rs` holds no SQL.
- **Polling a Fulfiller runs in its scope.** `progress.rs` asks it inside `ExtensionPoints::spawn_in`, so disabling it ends the call at once (ADR 0047).
- **A Fulfiller's try-again is retried in its scope.** `hand_off.rs` leaves the Request Approved and hands it off again after the wait, inside `ExtensionPoints::spawn_in`, up to 10 times in a row before failing it (ADR 0047).
- **Unit tests.** Each file's unit tests sit in its `*_tests.rs` beside it and share `testing.rs`.

### Neighbours

- Uses: `library` (`Contents`, `Metadata`, `library_with_movie`, `matched_episodes`, `TitleSources`), `events`, `plugins` (`ExtensionPoints`, `Answer`), `users` and `db`.
- Used by: the `api` parts for Requests and Plugins, `automation` (`who_requested`), `scanner` (deleting media, `RequestTitle`), `browse` (`RequestTitle`), `plugin_settings` (`list_qualities`, `check_qualities`, `replace_qualities`), startup in `lib.rs` and the `library` part `contents` (the ADR 0012 call).
