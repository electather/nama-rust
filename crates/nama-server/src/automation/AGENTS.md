## Automation

The Kernel's functions any Plugin calls (ADR 0048, ADR 0071), served to each Plugin by `Automation::serve`.

- **One owner for the Store:** only `store.rs` reads or writes `plugin_store`, tests included. A key is at most 256 bytes, a value at most 64 KiB, and a Plugin keeps at most 10 000 entries, Plugin-wide and per-User together, counted in the write that adds one; entries outlive disabling the Plugin, and deleting a User deletes theirs.
- **Permission first:** the contract's table of Kernel functions (`crates/nama-plugin-proto/AGENTS.md`) names each one's Plugin Permission; `FUNCTIONS` in `../automation.rs` binds each to its handler, and dispatch checks it before decoding the request or running the handler, refusing in plain words naming the Plugin and the Permission. A handler never checks a Permission itself.
- **Reuse, never write another area's tables:** a Kernel function answers through the operation that already has one owner, such as `Libraries::list` or `Scanner::start_delete`.
- **Held weakly by Plugins:** `Automation` holds the Scanner, `PluginSettings` and `Users`, which hold the Plugins, so each Plugin's Kernel functions hold it through a `Weak`; startup holds it until the Plugins have stopped, or the database never closes at shutdown.

### Neighbours

- Uses: `db`, `backup` (`Backups::write`), `library` (`Libraries`, `Contents`, `WatchStates`, `TitleTarget`, `title_roles`), `scanner` (`Scanner::start_delete`, `refresh_metadata`), `users` (`Users::list`, `Users::delete_expired`, `delete_plugins_personal_settings`), `requests` (`who_requested`), `watchlist` (`list`), `home_rows` (`forget_unvisited`), `plugin_settings` (`PluginSettings::take_gone`, and the Extension Points), `connections` (`delete_plugin`), and `playback` (`RunningPlans`, `Leftovers` and `SubtitleCache`) in tests.
- Used by: startup in `lib.rs`, through `ExtensionPoints::serve_kernel`.
