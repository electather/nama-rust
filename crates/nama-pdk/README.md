# nama-pdk

The Plugin development kit: everything a Nama Installed Plugin needs, from its Manifest to the traits and macros that export its Extension Points. An Installed Plugin is a WASM module the Server runs through [Extism](https://extism.org) (ADR [0006](../../docs/adr/0006-everything-is-a-plugin-runtime-plugins-are-extism-wasm.md)), built against the Plugin contract `nama.plugin.v1` (ADR [0041](../../docs/adr/0041-the-plugin-contract-stands-alone.md), [0043](../../docs/adr/0043-extension-points-are-contract-services.md)). `nama-pdk` re-exports the contract's messages from [`nama-plugin-proto`](../nama-plugin-proto), so a Plugin depends on `nama-pdk` alone.

Start from [`plugins/template`](../../plugins/template), a Notifier posting to a webhook, which CI builds so it keeps up with this crate. Fuller examples:

- [`plugins/tvdb`](../../plugins/tvdb): a title source.
- [`plugins/fanart`](../../plugins/fanart): a title source serving only artwork lists.
- [`plugins/sonarr`](../../plugins/sonarr): a Fulfiller, with Choice settings.
- [`plugins/telegram`](../../plugins/telegram): a Notifier and a Connector.
- [`plugins/ntfy`](../../plugins/ntfy): a Notifier reaching each User by their Personal settings.
- [`plugins/discord`](../../plugins/discord): a Notifier reaching each User by their Personal settings, and telling the Kernel when one is gone.
- [`plugins/webhook`](../../plugins/webhook): a Notifier posting to its channel alone.
- [`plugins/mdblist`](../../plugins/mdblist): a Home row source.
- [`plugins/opensubtitles`](../../plugins/opensubtitles): a subtitle source.

What the Kernel gives a running Plugin (its clock, its limits, its settings, its HTTP) is part of the contract and is listed once, in `nama-plugin-proto`'s README under [What the Kernel gives a Plugin](../nama-plugin-proto/README.md#what-the-kernel-gives-a-plugin). This README walks through writing a Plugin and links there for the rules; section 14 shows how a test stands in for the Kernel.

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
| `AUTOMATION` | `Automation`, `automation!` | `tick` | |
| `RECOGNISER` | `Recogniser`, `recogniser!` | | `recognise_title`, `recognise_file` |
| `BLOB_STORE` | `BlobStore`, `blob_store!` | `put`, `get`, `delete`, `list`, `exists` | |
| `HOME_ROW` | `HomeRowSource`, `home_row_source!` | | `shared_rows`, `user_rows` |
| `SUBTITLE_SOURCE` | `SubtitleSource`, `subtitle_source!` | `search`, `download` | |

A trait's optional methods have defaults, and its macro exports them all, except `title_source!`, `recogniser!` and `home_row_source!`, which export only the methods they list, since the Server asks nothing of a title source, Recogniser or Home row source that lacks the export:

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

A Fulfiller whose service can't act yet, such as Sonarr still adding a Show, returns `nama_pdk::TryAgainIn(duration)` from `fulfill`, `unfollow` or `stop_monitoring` rather than waiting: `fulfiller!` answers with `retry_after_seconds` set, and the Server asks again with the same input after that long, clamped to 1 to 300 seconds, while the Plugin stays enabled.

A Notifier is handed each notification with its `Recipient`: `Channel`, the channel in its settings; `Connection`, one User's Plugin Connection, which its Connector found; or `User`, one User who set its Personal settings, with their values (section 5). A Notifier that finds a recipient it can't serve, such as `User` without Personal settings, fails rather than tell the wrong people.

## 5. Settings

The Manifest's `settings` lists what an admin fills in, each a `SettingField` of a `SettingKind`. A Plugin reads a setting with `nama_pdk::setting(key)`, as text; what it holds when is in [What the Kernel gives a Plugin](../nama-plugin-proto/README.md#what-the-kernel-gives-a-plugin). A Libraries setting holds Library ids an admin picked from the Server's list; read it with `nama_pdk::setting_library_ids(key)`, and skip an id the Kernel no longer lists, since a Library may be deleted after the setting is saved.

A Plugin with a Choice setting, such as a quality profile, exports `check_settings()`, which the Server calls when an admin tests the settings, saves them with a Choice setting they set, or replaces a Fulfiller's Qualities with a non-empty list; a failing check refuses that save or replace, and a slow one holds it up. It checks them against the outside service and offers each Choice setting's values, built with `nama_pdk::checked_settings`:

```rust
use nama_pdk::{Buffa, CheckedSettings, FnResult, extism_pdk, plugin_fn};

#[plugin_fn]
pub fn check_settings() -> FnResult<Buffa<CheckedSettings>> {
    let profiles = [("1", "HD"), ("4", "Ultra HD")];
    Ok(Buffa(nama_pdk::checked_settings([("profile", profiles)])))
}
```

Settings each User sets for themself, such as a topic to be notified on, are Personal settings: the Manifest lists them in `personal_settings`, of the Text, Secret, Bool or Integer kind only, since a User's value must not widen the hosts the Plugin reaches nor name Libraries, and with keys `settings` doesn't use. They are never in the config. A Notifier reads one User's values from `Recipient::User`, as text:

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

## 6. Automation

An Automation Plugin asks the Kernel rather than being asked: the Kernel calls its `tick` every `tick_every_seconds` its Manifest sets. During any of its calls, any Plugin, an Automation or not, may call the Kernel's functions in `nama_pdk::kernel` (ADR 0071): its own Store (`store_get`, `store_put`, `store_delete`, each for the Plugin alone or for one User), `libraries`, `titles` (every page of a Library's titles, or one page with `titles_page`), `watch_states` and `users` (each User's id and display name) to read, and `delete_media` to delete a title's media files as an Admin's delete does. A file holding two Episodes goes with either, so `delete_media` takes the other titles the Plugin agrees to lose in `also`: the Kernel refuses a delete that would take along an Episode `also` doesn't name, naming each, and answers the titles that went along. The Store keeps at most 10 000 entries per Plugin, and refuses a new key at that cap. Each function needs a Plugin Permission, which the Manifest requests in `permissions` and the Admin grants when enabling the Plugin. Every Kernel function may be refused: one whose Permission the Admin didn't grant fails with the Kernel's reason, in plain words, as its error, and a Plugin carries on without it where it can. The functions and their Permissions are listed in [`automation.proto`](../../proto/nama/plugin/v1/automation.proto).

```rust,no_run
use nama_pdk::{
    Automation, Buffa, FnResult, Manifest, Permission, Tick, extism_pdk, extism_pdk::Error,
    kernel, plugin_fn, services,
};

#[plugin_fn]
pub fn manifest() -> FnResult<Buffa<Manifest>> {
    Ok(Buffa(Manifest {
        name: "Counter".to_owned(),
        description: "Counts its ticks.".to_owned(),
        points: vec![nama_pdk::point(services::AUTOMATION)],
        permissions: vec![Permission::PERMISSION_STORE.into()],
        tick_every_seconds: 3600,
        ..nama_pdk::manifest("counter", env!("CARGO_PKG_VERSION"))
    }))
}

struct Counter;

impl Automation for Counter {
    fn tick(_tick: Tick) -> Result<(), Error> {
        let count = kernel::store_get("ticks", None)?
            .and_then(|bytes| bytes.try_into().ok())
            .map_or(0, u64::from_le_bytes);
        kernel::store_put("ticks", None, &(count + 1).to_le_bytes())
    }
}

nama_pdk::automation!(Counter);
```

Off wasm32, as in a Plugin's unit tests, every Kernel function fails, since a test can't script them yet (section 14).

## 7. Recognising

A Recogniser proposes what a file is or which title an Item names, and the Server decides: it alone writes Matches and marks an Item Unmatched, and its own Matching is what it falls back to (ADR [0050](../../docs/adr/0050-recognisers-propose-what-a-file-is.md)). The Server asks a Library's Recognisers in the order the Admin sets, and takes the first answer that isn't empty.

- `recognise_file` is asked about a file the Server's name parser made no Item from, such as `Show - 137.mkv`, an Episode numbered from the start of its Show. It answers a `RecognisedMovie` or `RecognisedEpisodes` (a Show's title, a Season and its Episode numbers), which the Server then Matches as usual.
- `recognise_title` is asked about an Item the title sources' search couldn't settle, with its title, its files and the candidates the search found. It answers the title's ids, which the Server takes only in a namespace one of the Library's title sources serves, and fetches as it would an ID Hint.

An empty answer means the Recogniser can't tell; a failure leaves the Item or file undecided until the next Scan. A method left out answers empty, but `recogniser!` exports only the methods it lists, so list only those the Recogniser serves:

```rust
use nama_pdk::{
    FileRecognised, FileToRecognise, Manifest, RecognisedEpisodes, Recogniser, extism_pdk::Error,
    services,
};

fn manifest() -> Manifest {
    Manifest {
        points: vec![nama_pdk::point(services::RECOGNISER)],
        ..nama_pdk::manifest("absolute-episodes", env!("CARGO_PKG_VERSION"))
    }
}

struct AbsoluteEpisodes;

impl Recogniser for AbsoluteEpisodes {
    fn recognise_file(file: FileToRecognise) -> Result<FileRecognised, Error> {
        // Read `file.path`; this one says every file is the first Episode.
        let episodes = RecognisedEpisodes {
            show_title: file.path,
            season: 1,
            episodes: vec![1],
            ..RecognisedEpisodes::default()
        };
        Ok(FileRecognised {
            recognised: Some(episodes.into()),
            ..FileRecognised::default()
        })
    }
}

nama_pdk::recogniser!(AbsoluteEpisodes, [recognise_file]);
```

## 8. Storing blobs

A blob store keeps the Server's own stored files, such as its images' originals, Custom Images and Plugin logos, in an outside store such as S3; it never holds media files (ADR [0052](../../docs/adr/0052-the-kernels-stored-files-live-in-a-blob-store.md)). The Admin picks it as a Server setting, and local disk is the default. Every method is required, and `blob_store!` exports them all as `blob_put`, `blob_get`, `blob_delete`, `blob_list` and `blob_exists`.

A key is `/`-separated, at most 1024 bytes, of ASCII letters, digits and `-_.%/`. One call carries one object of at most `nama_pdk::BLOB_MAX_BYTES`, 20 MiB: the Server never puts a larger one, and a store may refuse one. A missing key is no failure: `get` answers `None`, and `delete` succeeds. `list` answers at most 1000 keys a page, with an empty `next_page_token` on the last. A failure fails the Server's read or write, and a blob call may run for 60 seconds.

```rust
use nama_pdk::{BlobPage, BlobStore, Manifest, extism_pdk::Error, services};

fn manifest() -> Manifest {
    Manifest {
        points: vec![nama_pdk::point(services::BLOB_STORE)],
        ..nama_pdk::manifest("bucket", env!("CARGO_PKG_VERSION"))
    }
}

struct Bucket;

impl BlobStore for Bucket {
    fn put(key: String, bytes: Vec<u8>) -> Result<(), Error> {
        if bytes.len() > nama_pdk::BLOB_MAX_BYTES {
            return Err(Error::msg(format!("{key} is over 20 MiB")));
        }
        // Send `bytes` to the store with `nama_pdk::http_request`.
        Ok(())
    }

    fn get(_key: String) -> Result<Option<Vec<u8>>, Error> {
        Ok(None)
    }

    fn delete(_key: String) -> Result<(), Error> {
        Ok(())
    }

    fn list(_prefix: String, _page_token: String) -> Result<BlobPage, Error> {
        Ok(BlobPage::default())
    }

    fn exists(_key: String) -> Result<bool, Error> {
        Ok(false)
    }
}

nama_pdk::blob_store!(Bucket);
```

## 9. HTTP

Send requests with `nama_pdk::http_request`, which takes the error the call fails with when nothing answers and answers a `nama_pdk::Response`. `nama_pdk::extism_pdk` has no `config` or `http`, so settings and requests always go through `nama-pdk`, which a test can stand in for (section 14). Which hosts a Plugin may reach (`allowed_hosts` and its URL settings' hosts) is in [What the Kernel gives a Plugin](../nama-plugin-proto/README.md#what-the-kernel-gives-a-plugin). Read the time with `nama_pdk::now`, never `SystemTime::now()`.

Plugins are not given HTTP response headers today: a response carries its status and body only. A later stage may add them, so don't build on either.

## 10. The call budget

Each call has a memory and a time limit, and three crashes in a row disable the Plugin; the numbers and what counts as a crash are in [What the Kernel gives a Plugin](../nama-plugin-proto/README.md#what-the-kernel-gives-a-plugin). Keep each call short: hand work to the outside service and report on it later, as a Fulfiller's `progress` does.

## 11. Errors

An export's error is its message to people. `manifest()`'s gets the Plugin refused and `check_settings()`'s is shown to the admin testing, saving settings or replacing Qualities, and the trait docs' `# Errors` say where each point's goes, such as `fulfill`'s, which becomes the reason a Request shows as Failed. Write it as a sentence an admin can act on. A panic reaches the Server with its message, not as a bare trap.

## 12. Logging

Log with `nama_pdk::{debug!, info!, warn!, error!}`, which take `format!`-style arguments:

```rust
nama_pdk::info!("Radarr took Request {} for TMDB id {}", 12, 603);
```

The Server logs each line under the target `nama::plugin`, naming your Plugin, with the values of its secret settings blanked. That catches a secret only as it is stored, so **never log a setting's value, a token or a response's body**: log ids, counts and statuses. A line over 2 KiB is cut, and a Plugin writing more than a burst of 50 lines, then 10 a second, has the rest dropped. `debug!` shows only when the admin asks for it, such as with `RUST_LOG=info,nama::plugin=debug`. In `manifest()`, `check_settings()` and `check_personal_settings()`, and in native unit tests, the lines go nowhere. Extism's own log macros don't build with `nama_pdk::extism_pdk`; the details are in [What the Kernel gives a Plugin](../nama-plugin-proto/README.md#what-the-kernel-gives-a-plugin).

## 13. Building and installing

```sh
rustup target add wasm32-unknown-unknown
cargo build --release --target wasm32-unknown-unknown
```

Copy `target/wasm32-unknown-unknown/release/nama_plugin_hello.wasm` into `<data_dir>/plugins/`, where `data_dir` is the Server's config setting. The Server reads that folder when it starts and whenever the Plugins are listed; an admin then enables the Plugin on its page under Plugins.

## 14. Testing a Plugin

A Plugin's native unit tests call its Extension Point methods, such as `fulfill`, `deliver` or `put`, the way the Kernel would, through `nama_pdk::testing` (ADR [0065](../../docs/adr/0065-a-plugin-tests-its-conversation-through-nama-pdk.md)). `testing::with(&mut kernel, || ...)` gives the call a `testing::Kernel` holding the Plugin's settings and a handler answering its HTTP requests; afterwards `kernel.sent()` lists the requests it sent, in order, with their bodies. A request sent with no handler panics, so it can't reach the Plugin as an error it might retry or swallow, and a handler should panic too at one it didn't expect, as a queue does once it runs out; a handler answering `Err` is a host that can't be reached, and the request fails with the Plugin's own words for that. Those words come back to the Plugin, where the Server would end the call with them, so pass them up with `?` for the test to see what the Server would. The Kernel doesn't check the Manifest's allowed hosts, which the Server's own tests cover. `testing` is unstable: the Kernel's functions can't be scripted yet.

```rust
use nama_pdk::{
    Response,
    extism_pdk::{Error, HttpRequest},
    testing::{Kernel, with},
};

/// What the Plugin does in a call: ask the service at its `url` setting how it is.
fn status() -> Result<u16, Error> {
    let url = nama_pdk::setting("url")?.unwrap_or_default();
    let request = HttpRequest::new(format!("{url}/status"));
    let response = nama_pdk::http_request(&request, None::<String>, "the service didn't answer")?;
    Ok(response.status_code())
}

let mut kernel = Kernel::new()
    .setting("url", "http://service.test")
    .http(|_request, _body| Ok(Response::new(503, "")));

assert_eq!(with(&mut kernel, status).ok(), Some(503));
assert_eq!(kernel.sent()[0].request.url, "http://service.test/status");
```

A handler is any closure, so a queue of answers is one that takes the next from a `Vec`, and an in-process fake of the service, such as S3's in [`plugins/s3/src/fake_s3.rs`](../../plugins/s3/src/fake_s3.rs), plugs in as one.
