## Automation

The Kernel's functions an Automation Plugin calls (ADR 0048), served to each Plugin by `Automation::serve`.

- **One owner for the Store:** only `store.rs` reads or writes `plugin_store`, tests included. A key is at most 256 bytes, a value at most 64 KiB, and a Plugin keeps at most 10 000 entries, Plugin-wide and per-User together, counted in the write that adds one; entries outlive disabling the Plugin, and deleting a User deletes theirs.
- **Permission first:** every Kernel function checks its Plugin Permission before anything else, against the one table in `Method::needs` in `../automation.rs`, and refuses in plain words naming the Plugin and the Permission.
- **Reuse, never write another area's tables:** a Kernel function answers through the operation that already has one owner, such as `Libraries::list` or `Scanner::delete_media`.
- **Held weakly by Plugins:** `Automation` holds the Scanner and the Extension Points, which hold the Plugins, so each Plugin's Kernel functions hold it through a `Weak`; startup holds it until the Plugins have stopped, or the database never closes at shutdown.

### Neighbours

- Uses: `db`, `library` (`Libraries`, `Contents`, `WatchStates`), `browse` (`Browse::doomed`), `scanner` (`Scanner::delete_media`), `users` (`Users::list`), `plugins` (`ExtensionPoints::name`).
- Used by: startup in `lib.rs`, through `ExtensionPoints::serve_kernel`.
