# nama-pdk

The Plugin development kit: everything a Nama Installed Plugin needs, from its Manifest to the traits and macros that export its Extension Points. An Installed Plugin is a WASM module the Server runs through [Extism](https://extism.org) (ADR [0006](../../docs/adr/0006-everything-is-a-plugin-runtime-plugins-are-extism-wasm.md)), built against the Plugin contract `nama.plugin.v1` (ADR [0041](../../docs/adr/0041-the-plugin-contract-stands-alone.md), [0043](../../docs/adr/0043-extension-points-are-contract-services.md)). `nama-pdk` re-exports the contract's messages from [`nama-plugin-proto`](../nama-plugin-proto), so a Plugin depends on `nama-pdk` alone.

Start from [`plugins/template`](../../plugins/template), a Notifier posting to a webhook, which CI builds so it keeps up with this crate. Fuller examples:

- [`plugins/tvdb`](../../plugins/tvdb): a title source.
- [`plugins/fanart`](../../plugins/fanart): a title source serving only artwork lists.
- [`plugins/sonarr`](../../plugins/sonarr): a Fulfiller, with Choice settings.
- [`plugins/telegram`](../../plugins/telegram): a Notifier and a Connector.

What the Kernel gives a running Plugin (its clock, its limits, its settings, its HTTP) is part of the contract and is listed once, in `nama-plugin-proto`'s README under [What the Kernel gives a Plugin](../nama-plugin-proto/README.md#what-the-kernel-gives-a-plugin). This README walks through writing a Plugin and links there for the rules.

## 1. A cdylib depending on nama-pdk

```toml
[package]
name = "nama-plugin-hello"
version = "0.1.0"
edition = "2024"

[lib]
crate-type = ["cdylib"]

[dependencies]
nama-pdk = "0.2.0"
```

Depend on nothing else from Nama: the contract is all a Plugin may see.

## 2. The Manifest

Every Plugin exports `manifest()`, which the Server calls once when it loads the Plugin. [`nama_pdk::manifest`](src/lib.rs) fills in the Plugin's `id` (which the Server keeps its settings under), its `version`, and its `contract`: a semver requirement on `CONTRACT_VERSION`, the version of the contract this `nama-pdk` was built against. Fill in the rest with struct update syntax: `name` and `description` for admins, and `logo_svg`, which must read on a light and a dark tile alike. The fields are documented in [`manifest.proto`](../../proto/nama/plugin/v1/manifest.proto).

```rust
use nama_pdk::{Buffa, FnResult, Manifest, extism_pdk, plugin_fn, services};

#[plugin_fn]
pub fn manifest() -> FnResult<Buffa<Manifest>> {
    Ok(Buffa(Manifest {
        name: "Hello".to_owned(),
        description: "Says hello.".to_owned(),
        points: vec![nama_pdk::point(services::NOTIFIER)],
        ..nama_pdk::manifest("hello", env!("CARGO_PKG_VERSION"))
    }))
}
```

`plugin_fn` expands to paths starting with `extism_pdk`, so import it as above. An export's doc comments and attributes are dropped, which is why the template keeps its exports in a module with an `#[expect]` for the lints that leaves.

## 3. Naming the points it serves

Each Extension Point is a service of the contract, named in the Manifest's `points` by its full name with a semver requirement on that service's own version. [`nama_pdk::point`](src/lib.rs) builds one from a name in `nama_pdk::services`, requiring the version in `SERVICES` this `nama-pdk` was built against or a compatible one.

The Server refuses a Plugin whose Manifest names a point it doesn't serve, wants a version of one it doesn't have, lacks an export a named point requires, or names no point at all. It says why in its log and on the Plugins page.

## 4. Serving a point

Each point has a trait and a macro that exports it. The trait docs in [`src/lib.rs`](src/lib.rs) hold each method's contract.

| Service | Trait and macro | Required | Optional |
| --- | --- | --- | --- |
| `MOVIE_FULFILLER`, `SHOW_FULFILLER` | `Fulfiller`, `fulfiller!` | `fulfill`, `progress`; `unfollow` for Shows | `stop_monitoring` |
| `NOTIFIER` | `Notifier`, `notifier!` | `deliver` | |
| `CONNECTOR` | `Connector`, `connector!` | `start_connection`, `poll_connection` | |
| `TITLE_SOURCE` | `TitleSource`, `title_source!` | | `search`, `movie`, `show`, `season`, `episode`, `image`, `artwork` |

A trait's optional methods have defaults, and its macro exports them all, except `title_source!`, which exports only the methods it lists, since the Server asks nothing of a title source that lacks the export:

```rust
use nama_pdk::{SearchQuery, SearchResults, TitleSource, extism_pdk::Error};

struct Tvdb;

impl TitleSource for Tvdb {
    fn search(_query: SearchQuery) -> Result<SearchResults, Error> {
        Ok(SearchResults::default())
    }
}

nama_pdk::title_source!(Tvdb, [search]);
```

A title source also sets the Manifest's `title_namespace`, the namespace it keeps titles under, and `title_lookup_namespaces`, any others it can look a title up by, and a Fulfiller its `movie_id_namespaces` or `show_id_namespaces`: see the contract's [README](../nama-plugin-proto/README.md) and its [Title ids](../nama-plugin-proto/README.md#title-ids).

```rust
use nama_pdk::{Manifest, services};

fn manifest() -> Manifest {
    Manifest {
        points: vec![nama_pdk::point(services::TITLE_SOURCE)],
        title_namespace: "tmdb".to_owned(),
        title_lookup_namespaces: vec!["imdb".to_owned(), "tvdb".to_owned()],
        ..nama_pdk::manifest("tmdb", env!("CARGO_PKG_VERSION"))
    }
}
```

Asked for a title by an id in a namespace it can't look titles up by, a title source answers with `unsupported_id` set, never a failure: the Server then asks the next source, while a failure means the source couldn't answer and is worth trying again.

A Notifier is handed each notification with its `Recipient`: `Channel`, the channel in its settings; `Connection`, one User's Plugin Connection, which its Connector found; or `User`, one User who set its Personal settings, with their values (section 5). A Notifier that finds a recipient it can't serve, such as `User` without Personal settings, fails rather than tell the wrong people.

## 5. Settings

The Manifest's `settings` lists what an admin fills in, each a `SettingField` of a `SettingKind`. A Plugin reads a setting with `extism_pdk::config::get(key)`, as text; what it holds when is in [What the Kernel gives a Plugin](../nama-plugin-proto/README.md#what-the-kernel-gives-a-plugin).

A Plugin with a Choice setting, such as a quality profile, exports `check_settings()`, which the Server calls when an admin tests the settings. It checks them against the outside service and offers each Choice setting's values, built with `nama_pdk::checked_settings`:

```rust
use nama_pdk::{Buffa, CheckedSettings, FnResult, extism_pdk, plugin_fn};

#[plugin_fn]
pub fn check_settings() -> FnResult<Buffa<CheckedSettings>> {
    let profiles = [("1", "HD"), ("4", "Ultra HD")];
    Ok(Buffa(nama_pdk::checked_settings([("profile", profiles)])))
}
```

Settings each User sets for themself, such as a topic to be notified on, are Personal settings: the Manifest lists them in `personal_settings`, of the Text, Secret, Bool or Integer kind only, since a User's value must not widen the hosts the Plugin reaches, and with keys `settings` doesn't use. They are never in the config. A Notifier reads one User's values from `Recipient::User`, as text:

```rust
use nama_pdk::Recipient;

/// The topic a User set, for a notification sent to them.
fn topic(recipient: &Recipient) -> Option<&str> {
    match recipient {
        Recipient::User(user) => user.personal_settings.get("topic").map(String::as_str),
        Recipient::Channel(_) | Recipient::Connection(_) => None,
    }
}
```

## 6. HTTP

Send requests with `nama_pdk::http_request`, which works like `extism_pdk::http::request` but takes the error the call fails with when nothing answers. Which hosts a Plugin may reach (`allowed_hosts` and its URL settings' hosts) is in [What the Kernel gives a Plugin](../nama-plugin-proto/README.md#what-the-kernel-gives-a-plugin). Read the time with `nama_pdk::now`, never `SystemTime::now()`.

Plugins are not given HTTP response headers today: a response carries its status and body only. A later stage may add them, so don't build on either.

## 7. The call budget

Each call has a memory and a time limit, and three crashes in a row disable the Plugin; the numbers and what counts as a crash are in [What the Kernel gives a Plugin](../nama-plugin-proto/README.md#what-the-kernel-gives-a-plugin). Keep each call short: hand work to the outside service and report on it later, as a Fulfiller's `progress` does.

## 8. Errors

An export's error is its message to people. `manifest()`'s gets the Plugin refused and `check_settings()`'s is shown to the admin testing, and the trait docs' `# Errors` say where each point's goes, such as `fulfill`'s, which becomes the reason a Request shows as Failed. Write it as a sentence an admin can act on. A panic reaches the Server with its message, not as a bare trap.

## 9. Building and installing

```sh
rustup target add wasm32-unknown-unknown
cargo build --release --target wasm32-unknown-unknown
```

Copy `target/wasm32-unknown-unknown/release/nama_plugin_hello.wasm` into `<data_dir>/plugins/`, where `data_dir` is the Server's config setting. The Server reads that folder when it starts and whenever the Plugins are listed; an admin then enables the Plugin on its page under Plugins.
