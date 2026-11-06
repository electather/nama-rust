# Spike: Extism + protobuf plugin round-trip (#11)

A throwaway prototype that tests ADR 0006 before M3 builds the plugin runtime. It isn't a member of the Nama workspace (it has its own `[workspace]`), and nothing here is meant to be merged into the real crates.

| Path | What it is |
|---|---|
| `proto/spike.proto` | The messages the host and guest exchange |
| `shared/` | buffa types generated from the proto, plus `Buffa<T>`, the Extism encoding for them |
| `guest/` | The Installed Plugin, built for `wasm32-unknown-unknown`. The `v2` feature builds a "new version" |
| `host/` | Loads the guest and runs one experiment per question, printing what happened |

Run it with `./run.sh`. It needs `protoc` and the `wasm32-unknown-unknown` target. It builds the guest twice, then runs the host in release mode (about 40 s for the 2,000 uncached compiles).

## Findings

Measured with extism 1.41.0, extism-pdk 1.4.1 and buffa 0.9.2, on a 10-core aarch64 Linux VM. The guest is a 162 KB release build.

1. **Protobuf over Extism works, with almost no boilerplate.** The issue asked about prost, but the spike uses buffa: buffa is what `nama-proto` uses (`connectrpc` requires it), so plugin messages can come from the same schema as the client API. `extism-convert` ships a `Prost<T>` wrapper but nothing for buffa, so `shared/src/lib.rs` adds `Buffa<T>`: about 20 lines implementing `ToBytes` and `FromBytesOwned`. With it, an export is an ordinary `#[plugin_fn] fn greet(Buffa(req): Buffa<GreetRequest>) -> FnResult<Buffa<GreetResponse>>`, and the host calls `plugin.call::<_, Buffa<GreetResponse>>("greet", Buffa(req))`. No new macro is needed: `nama-pdk` only has to re-export `plugin_fn` and provide `Buffa<T>`. One gap: a guest panic reaches the host as a bare `unreachable` trap with a wasm backtrace, but without the panic message. `nama-pdk` should install a panic hook that passes the message on through Extism's error or log functions.
2. **Host HTTP with an allowlist works.** The guest GETs a mock Sonarr on `127.0.0.1` through `extism_pdk::http::request`. When the manifest allows `127.0.0.1`, the call succeeds with status 200 and the mock's JSON body. The same server reached as `localhost` is refused with `HTTP request to http://localhost:…/ is not allowed`. A manifest with no allowed hosts refuses everything. Extism matches the URL's host name against glob patterns. Ports and schemes aren't part of the match (read from extism's `pdk.rs`, not exercised here), so allowing a host allows every port on it. The allowlist is set per loaded Plugin, so the Server can add hosts at load time. Sonarr and Radarr need that, because their address is something an admin types into the Plugin's settings and isn't known when the package is built.
3. **Load, unload and reload at runtime work, and nothing leaks.** 2,000 rounds of read `.wasm` from disk → compile → call → drop, alternating v1 and v2, each answered with the right version. A compile without any cache takes **17.5 ms** per load. With wasmtime's default on-disk cache warm it's **1.6 ms**. RSS went from 52 MB after 50 rounds to 60 MB after 2,000. Run with `MALLOC_ARENA_MAX=1 ./run.sh` and it stays flat (30.6 → 31.2 MB), so the growth is glibc arena fragmentation, most likely from wasmtime compiling on many threads, not a leak. Instantiating from a `CompiledPlugin`, calling once and dropping takes **42 µs**, and RSS was flat over 50,000 rounds.
4. **Timeouts and crashes leave the host healthy.** With a 100 ms manifest timeout, an endless loop is stopped after **104 ms** with a `timeout` error. A guest panic comes back as a trap error. In both cases the host carries on, and the same instance even answers the next call. The Server should still throw an instance away after a timeout or trap, because the guest may have been interrupted halfway through changing its own state. Re-instantiating from the `CompiledPlugin` costs under 42 µs (that figure includes one call).
5. **A call costs about 2 µs.** A `greet` round trip with a small message (encode, copy in, call, copy out, decode) takes **2.2 µs** without a timeout and **2.3 to 2.8 µs** with one, measured over 20,000 warm calls. That's cheap for an Intercept or Provider called once per Request or per Match, but at 100,000 files a per-file call adds about 0.2 s plus whatever the guest does. That confirms ADR 0006's hot-path rule: batched Observe events and coarse Intercepts only.
6. **Non-Rust guest: not tried.** No JS or Go toolchain for Extism (`extism-js`, TinyGo) is in this environment, and the question is optional. On paper the JS PDK would get its message types from `buf generate` with `@bufbuild/protobuf`, the same schema run through a different plugin, and would read and write raw bytes through the PDK's input and output functions.

Notes for M3:

- `Plugin::call` takes `&mut self`, so one instance serves one call at a time. For concurrent calls, the Server should keep one `CompiledPlugin` per Installed Plugin and either instantiate per call (42 µs) or use `extism::Pool`.
- `Plugin::new` uses wasmtime's on-disk compilation cache by default (`~/.cache/wasmtime`). The Server should either choose that location itself or turn the cache off and keep `CompiledPlugin`s in memory.

## Recommendation

Keep ADR 0006 as written. Extism with protobuf answers every question cheaply, so the extra machinery of the wasmtime Component Model isn't justified. The ADR gains one clarification, from finding 2: allowed HTTP hosts are host names, with no ports. It also gains one decision, which a human should confirm: hosts from a Plugin's settings are added when the Server loads it. Without that, a Sonarr Plugin couldn't reach a Sonarr whose address the admin chooses. That's acceptable because Plugins are trusted code (ADR 0006) and the admin entered the host.
