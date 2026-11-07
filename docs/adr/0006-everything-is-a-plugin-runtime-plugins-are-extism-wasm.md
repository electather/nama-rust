# Everything outside the Kernel is a Plugin; installed Plugins are Extism WASM

Following deepseek-harness's "everything is a plugin" design, every feature outside a small fixed Kernel is a Plugin behind an Extension Point. The Kernel is storage, scanning, streaming/transcoding, auth sessions and the plugin registry. Built-in Plugins are Rust crates compiled into the Server. Installed Plugins are `.wasm` files loaded at runtime through Extism and wrapped by one adapter that implements the same Rust traits, so the Kernel can't tell the two kinds apart. Plugins are trusted code. WASM was chosen for **portability**, not sandboxing: one `.wasm` file runs on every platform a self-hoster uses and survives Server upgrades.

## Considered Options

- **Compile-time crates only**: rejected, because users run a prebuilt binary and must be able to install Plugins at runtime.
- **Native dynamic libraries**: rejected. Rust has no stable ABI, so every Plugin would need a separate build for each platform and each Server version (the Jellyfin breakage problem, made worse). `abi_stable` hasn't been released since 2023.
- **Embedded JS (rquickjs)**: rejected. Plugins could only be written in JS, and the bindings would be maintained by hand.
- **wasmtime Component Model + WIT**: rejected for now as more machinery than Extism. Plugin messages are protobuf generated from the same schema as the client API, so nothing is written by hand.
- **In-process, unsandboxed, everything replaceable (deepseek-harness as-is)**: rejected. Its Kernel is replaceable; ours isn't.

## Consequences

- Extension Points are declared Observe, Intercept or Provide. Hot paths (scanning, segment serving) expose only batched Observe events and per-item or per-session Intercepts.
- Every registration a Plugin makes belongs to that Plugin, so install, enable and remove work without a restart.
- A manifest, which the `.wasm` exports as a protobuf message from a `manifest()` function so a Plugin is one file, declares the supported `nama-api` version range, a settings schema and allowed HTTP hosts. Hosts are host names (globs allowed) with no port, which is how Extism matches them. Hosts the admin enters in a Plugin's settings, such as a Sonarr URL, are added when the Server loads it (spike #11). Intercepts have timeouts, and a crashing Plugin is disabled, not fatal.
- Installed Plugins depend only on host capabilities, never on each other.
- Built-in Plugins are separate crates (in `plugins/`), not modules of the Server crate. That's the only way to make the compiler stop them reaching into Kernel internals, which is what "everything is a Plugin" depends on.
