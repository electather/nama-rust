## Automation

The Kernel's functions an Automation Plugin calls (ADR 0048), made per Plugin by `Automation::kernel`.

- **One owner for the Store:** only `store.rs` reads or writes `plugin_store`, tests included. A key is at most 256 bytes and a value at most 64 KiB; entries outlive disabling the Plugin, and deleting a User deletes theirs.
- **Permission first:** every Kernel function checks its Plugin Permission before anything else, against the one table in `Method::needs` in `../automation.rs`, and refuses in plain words naming the Plugin and the Permission.
- **Reuse, never write another area's tables:** a Kernel function answers through the operation that already has one owner, such as `Libraries::list` or `Scanner::delete_media`.

### Neighbours

- Uses: `db`, `library` (`Libraries`); later `browse` and `scanner`.
- Used by: startup in `lib.rs`, through `ExtensionPoints::serve_kernel`.
