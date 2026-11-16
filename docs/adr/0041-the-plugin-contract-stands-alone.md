# The Plugin contract stands alone

What an Installed Plugin is built against is the Plugin contract: the `nama.plugin.v1` schema in [`proto/nama/plugin/`](../../proto/nama/plugin/), the exports a Server calls, and what the Kernel gives a Plugin. It imports nothing from the client API, `nama.v1`. Where both need the same thing, such as a Request, an Event, a download or a setting's kind and value, the contract has a message of its own holding what a Plugin needs, and the Kernel converts where it calls an Installed Plugin. Its generated code lives in its own crate, [`nama-plugin-proto`](../../crates/nama-plugin-proto/), licensed MIT OR Apache-2.0 like the other contracts (ADR 0009); `nama-pdk` and Installed Plugins depend on it and not on `nama-proto`, which keeps the client API. The contract has a version of its own, a constant in that crate, which a Manifest's requirement is matched against, so a change under `proto/nama/plugin/` needs no entry in [`proto/BREAKING.md`](../../proto/BREAKING.md): that log (ADR 0032) records breaks of the client API only. The contract's `Event` holds the kinds a Plugin has needed so far, what a Scan did to a Library and what happened to a Request; Request progress, which changes every minute, and Followed Show and Playback events reach no Installed Plugin until one needs them, and adding a kind is a patch bump. Third parties build Plugins against a public contract while the client API changes for the web app's needs: sharing messages made every client change a Plugin change, shipped the whole client API to every Plugin, and tied two versions together. This reverses the sharing of `SettingKind` and `SettingValue` made on 2026-11-11 (#625).

## Considered Options

- **Keep importing `nama.v1`**: rejected. Every client API change stays a Plugin change, and the contract's version can't say what a Plugin depends on.
- **One crate with a feature for the contract**: rejected. The contract's code would share the client API's crate version, and a feature left on would bring the client API back to Plugins.
- **A copy of every client message in the contract**: rejected. Copies follow the client's shape, so a Plugin would still change whenever the web app's needs do, and it would carry fields it never reads.

## Consequences

- One idea has two messages, kept apart on purpose, with the conversion code in the Server.
- Built-in Plugins still see client API messages through `nama-plugin-api` until Extension Points become proto services.
- A check keeps `proto/nama/plugin/` from importing `nama.v1`.
- The Kernel hands Request progress to Built-in Notifiers only; an Installed Notifier is not called for it. Telegram, the only one, posted nothing for it.
