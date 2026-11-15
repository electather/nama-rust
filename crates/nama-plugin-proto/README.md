# nama-plugin-proto

Rust types for the Plugin contract, generated from [`proto/nama/plugin/`](../../proto/nama/plugin) (ADR 0041). `nama.plugin.v1` is the module `nama_plugin_proto::nama::plugin::v1`, holding the messages the Kernel and its Installed Plugins exchange, such as the `Manifest`. `nama-pdk` re-exports this crate, so a Plugin depends on `nama-pdk` alone.

`CONTRACT_VERSION` is the Plugin contract's version, which an Installed Plugin's `Manifest.contract` requirement is matched against. It has a number of its own, separate from this crate's version in `Cargo.toml`; `AGENTS.md` says when it changes.

The contract is more than the messages: `CONTRACT_VERSION` also covers what the Kernel gives an Installed Plugin at run time, below.

Each Extension Point an Installed Plugin can serve is a service of the contract, such as `nama.plugin.v1.NotifierService`, whose rpcs name the exports the Kernel calls (ADR 0043). Each has a version of its own, in `SERVICES`; the `services` module holds their full names. A Manifest names the points it serves in `points`, each by its service's full name and a semver requirement on its version.

`nama.plugin.v1.TitleSourceService`, in [`title_source.proto`](../../proto/nama/plugin/v1/title_source.proto), is a title source the Kernel searches and fetches Movies and Shows from to Match a Library's files. Every one of its exports (`title_search`, `title_movie`, `title_show`, `title_season`, `title_episode`, `title_image` and `title_artwork`) is optional: the Kernel asks a source only for the ones it has. Its Manifest names in `title_namespace` the id namespace it keeps titles under, such as `tvdb`, and in `title_lookup_namespaces` any others it can look a title up by, such as `imdb`; the Kernel refuses one naming the service without a well-formed namespace in the first, or with a malformed one in the second. Asked by an id in a namespace it can't use, a source answers with `unsupported_id` set, and the Kernel asks the next source. When it stores a Match, the Kernel asks every source in the Library's order that has `title_artwork` for its artwork list and picks the Poster, Backdrop and Title Logo per kind (ADR 0045).

`nama.plugin.v1.NotifierService`, in [`notifier.proto`](../../proto/nama/plugin/v1/notifier.proto), delivers notifications about Requests. A `Delivery` goes to the Plugin's own channel, to one User's Plugin Connection, or, for a Plugin whose Manifest declares `personal_settings`, to a `UserRecipient`: a User who set the Plugin's Personal settings, with their values, such as their own webhook or topic.

## Personal settings

A Manifest's `settings` are Plugin-wide: an admin sets them once. Its `personal_settings` (ADR 0046) are settings each User sets for themself. Only the TEXT, SECRET, BOOL and INTEGER kinds are allowed, since a User's value must not widen the hosts a Plugin reaches: the Kernel refuses a Plugin with a URL or CHOICE Personal setting, or with a key that repeats or that `settings` also names. A Plugin never finds them in its Extism config; the Kernel hands one User's values, as text, only in a call for that User, such as a `Delivery` to `UserRecipient`.

## Versions

The contract crates, `nama-plugin-proto`, `nama-pdk` and `nama-plugin-api`, move together: their version in `Cargo.toml` changes with each release, separately from `CONTRACT_VERSION` and the service versions in `SERVICES`.

- **0.3.0 (unreleased)**: `NotifierService` 1.2.0: for a `UserRecipient` that answers `recipient_gone`, the Kernel deletes that User's Personal settings for the Plugin.
- **0.2.0**: `TitleSourceService` 1.1.0 adds `Manifest.title_lookup_namespaces`, the `unsupported_id` answers and the `Artwork` rpc with `ArtworkAsked`, `ArtworkAnswer`, `Artwork` and `ArtworkKind`. `NotifierService` 1.1.0 adds `UserRecipient`, and the Manifest adds `personal_settings`.

## Title ids

A title's ids reach a Plugin as `repeated TitleId`, each a namespace and a value, defined in [`proto/nama/plugin/v1/request.proto`](../../proto/nama/plugin/v1/request.proto) (ADR [0042](../../docs/adr/0042-a-titles-id-is-a-namespace-and-a-value.md)). A namespace is one or more of a-z and 0-9; a value is never empty. A title has at most one id per namespace, and any one of them identifies it. `tmdb`, `imdb` and `tvdb` are well known, and a Plugin may bring any other. The Kernel never interprets a value, so a Plugin that needs a number parses it itself. `nama_pdk::title_id` reads the value in one namespace.

The Manifest's `movie_id_namespaces` and `show_id_namespaces` (fields 11 and 12 in [`manifest.proto`](../../proto/nama/plugin/v1/manifest.proto)) name the namespaces a `MovieFulfillerService` or `ShowFulfillerService` can find a title by. Any one is enough, and an empty list means any title. The Kernel refuses at load a Manifest naming a malformed namespace there. It never hands such a Fulfiller a Request whose title has none of them: it fails the Request instead, showing the missing namespaces.

## What the Kernel gives a Plugin

The Server sets this up in [`crates/nama-server/src/plugins/wasm/load.rs`](../nama-server/src/plugins/wasm/load.rs), mostly in `compile`. It calls an Installed Plugin in three ways: `manifest()` once, when it loads the Package; `check_settings()` when an admin tests settings; and the exports of the Extension Points its Manifest names, such as `fulfill`, while it is enabled.

- **A clock.** The function `now` in the Wasm import namespace `nama` takes no arguments and returns an `i64`: the milliseconds since the Unix epoch by the Server's clock. A Plugin needs it because `SystemTime::now()` panics on `wasm32-unknown-unknown`. Every call has it.
- **128 MiB of memory and 5 seconds per call**, the memory counting the Plugin's own and what Extism keeps for it. A call past either limit fails. What that costs depends on the call: past one, `manifest()` gets the Plugin refused, and `check_settings()` fails that check.
- **Three crashes in a row disable the Plugin.** Only an Extension Point's export crashes: when it runs past a limit or traps, not when it returns an error. Each Extension Point counts its own crashes, except that a Movie and a Show Fulfiller count as one.
- **Settings as text.** `check_settings()` and the Extension Points' exports find each setting in the Extism config under its key, as text: a bool as `true` or `false`, a number in decimal. A setting holds what the admin set, or its Manifest default if the admin set nothing; one with neither is not in the config. `check_settings()` gets the settings being tested, which may be unsaved. `manifest()` gets no settings. Personal settings are never in the config; see above.
- **HTTP to named hosts only.** A Plugin may send HTTP requests only to the hosts its Manifest's `allowed_hosts` names and the host of each of its URL settings: the saved settings for an Extension Point's exports, the settings being tested for `check_settings()`. `manifest()` may send none.
- **No HTTP response headers.** The Kernel doesn't pass a response's headers to the Plugin: `compile` leaves Extism's `with_http_response_headers` off.

## Generated code is checked in

The code under `src/generated/` is written by `buf generate` and committed, for the reasons in [`crates/nama-proto/README.md`](../nama-proto/README.md). Don't edit it by hand. To regenerate after a schema change, install the tools as that README says, then run `buf generate` in `proto/`; one run writes both crates and the web app's code.
