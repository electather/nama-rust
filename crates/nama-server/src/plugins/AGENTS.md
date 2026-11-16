## Plugins

The Registry, how the Kernel calls Plugins at Extension Points, and the Built-in and Installed Plugins (ADR 0006).

- **Plugin code runs contained.** Every call into a Plugin, its `Drop` included, goes through `contain_sync` (or its async twin) in `plugins.rs`, outside the registry lock. A Provider's error is Plugin code too: the Registry formats it into a `Failure`, blanks the Plugin's secret settings in it and drops it, all inside containment, so a caller gets only the `Failure` and never formats or drops a Plugin's error itself. A caller that branches on the kind of failure maps the error to a Kernel-side kind with `ProviderError`. The one exception is an Authenticator hashing or checking a password on a blocking thread (`blocking` in `users.rs`): a panic there fails that call and leaves the Plugin enabled, since disabling the only Authenticator on a password anyone can choose would lock every User out.
- **A Kernel task that calls one Plugin repeatedly runs in its scope.** Start it with `ExtensionPoints::spawn_in`, so disabling, re-enabling or shutdown cancels and joins it before the Plugin is dropped (ADR 0047). `Registry::disable` waits for that join, so a caller inside the Plugin's own call or scope uses `disable_now`.
- **An Automation ticks at enable, then every `tick_every`** (`tick.rs`), in its Plugin's scope, so an Admin sees it act at once and disabling ends the loop.
- **The Kernel can't tell a Built-in from an Installed Plugin.** An Installed Plugin runs through Extism behind the same `Plugin` trait (`wasm.rs`); only listing and saving a crash tell the two kinds apart.
- **Private modules.** The area's files are private modules; neighbours import only what `plugins.rs` re-exports, and items inside default to `pub(super)`. `wasm.rs` and `wasm/` hold everything Extism.
- **Each method has its own time limit.** Its row in `wasm/services.rs` carries it (`CALL_TIMEOUT`, or `TICK_TIMEOUT` for a tick), and the watchdog in `wasm/calls.rs` counts only the Plugin's own time against it: a `nama` import pauses the call's clock while the Kernel answers.
- **The Kernel's functions are table rows.** Each `nama` import an Installed Plugin calls is one row of `FUNCTIONS` in `wasm/automation.rs`, which only decodes, calls `nama_plugin_api::automation::Kernel` and encodes. Whether the Plugin may call it, its Permission check, is the `automation` area's, not this one's.
- **What the Kernel gives an Installed Plugin is contract.** The limits on a call, the `nama` functions and how settings and HTTP reach it (`wasm/load.rs`) follow `crates/nama-plugin-proto/AGENTS.md`.

### Neighbours

- Uses: `images` (cleaning a Manifest's logo).
- Used by: `plugin_settings` (the `Registry`), startup in `main.rs`, and, through `ExtensionPoints`, `library::title_source`, `matching`, `scanner`, `library::artwork`, `requests`, `events`, `connections`, `users` and the `api` part for Plugins.
