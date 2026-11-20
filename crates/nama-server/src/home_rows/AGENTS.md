## Home rows

The answers Plugins give ahead of time through `HomeRowSource`, stored per Plugin and scope and refreshed in the background, so home never waits on a Plugin (ADR 0058, ADR 0072): Home rows (`rows.rs`) and Featured suggestions (`featured.rs`).

- **One lifecycle.** Only `Refreshes` in `refreshes.rs` reads or writes `home_row_refreshes` and `featured_refreshes`, tests of it aside, and it alone puts off, refreshes, guards the replace of, and forgets a stored answer. They aren't in `OWNED_TABLES`, since the database tests' upgrade test names them (ADR 0059).
- **One adapter per answer.** Each answer's table (`home_rows`, `featured_suggestions`) is read and written only by its adapter, the file named above, tests of it aside. A new answer a Plugin gives ahead of time joins as another `Adapter` of `Refreshes`, never with a put-off of its own.

### Neighbours

- Uses: `db`, `library` (`Libraries`, `Contents`), `page`, `plugins` (`ExtensionPoints`, `Answer`, `without_secrets`), `requests` (`listed_movie`, `listed_show`, `open_movie_request`), `server_settings` (`HomeRowId`) and `users` (`User`, a User's Personal settings).
- Used by: the `api` parts for browse and the Server (`HomeRows`), `browse` (`Suggested`) and startup in `lib.rs`.
