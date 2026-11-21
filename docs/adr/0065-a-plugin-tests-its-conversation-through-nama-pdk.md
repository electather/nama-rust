# A Plugin tests its conversation through nama-pdk

_Amended by [ADR 0079](0079-plugins-declare-jobs-and-the-kernel-runs-them-on-cron.md): Jobs replace the Tick, so a `RunJob` carries the Kernel's clock._

What the Kernel gives an Installed Plugin (HTTP, settings and the Kernel's functions) reaches it only through `nama-pdk`, which has two ends: Extism on wasm32, and off wasm32 an in-process one a test installs for one call, scripting the outside service's answers and the Plugin's settings and reading back what the Plugin sent. A Plugin's tests then call its Extension Point methods, such as `fulfill`, `deliver` or `tick`, and the conversation is tested in the Plugin's own crate, not through a wasm build and a running Server with fake services. To keep Plugins from going round it, `nama_pdk::extism_pdk` no longer exposes Extism's `config`, `http` or `HttpResponse`: settings come from `nama_pdk::setting` and an answer is `nama-pdk`'s own `Response`, since Extism's can't be built off wasm32. It is public kit surface, unstable while the `0.x` versions last, because third-party Plugins need the same tests the bundled ones have (#1198).

## Considered Options

- **The whole seam at once**: rejected. HTTP and settings go first, proved on Sonarr, Telegram and S3; the Kernel's functions, which off wasm32 refuse today, follow once that shape holds, and Cleanup's `tick` with them.
- **Leave it, and let a Plugin thread closures as S3 did**: rejected. One Plugin in ten did, and the rest tested helpers while their bugs lived in the callers.
- **Fake Extism's host functions**: rejected. Calling them off wasm32 fails at link time, and their types aren't ours to build.

## Consequences

- No clock: a Tick carries the Kernel's clock, and `nama_pdk::now` keeps its native branch for S3's signing.
- An answer the test didn't script panics rather than reaching the Plugin as an error a Plugin might retry or swallow. A test can script a host that can't be reached.
- The in-process end doesn't check a Manifest's allowed hosts; the Kernel does, and the Server's tests cover it. The Server's own test Plugin calls Extism's HTTP directly for that, since `nama_pdk::http_request` puts a Plugin's "didn't answer" in place of the Kernel's refusal.
- A Server-level test of a Plugin's conversation is deleted only once the Plugin's own test asserts the same requests. Tests of what the Kernel does with a Plugin stay.
- `Registry::tick_now` and `tick_only_on_demand` go once the Kernel's functions are scriptable and Cleanup's tests no longer need a Server.
