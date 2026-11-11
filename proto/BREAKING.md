# Declared schema breaks

CI runs `buf breaking` against main on every change (ADR 0007). A change that breaks the schema on purpose declares it here, and CI then prints the breaks and passes (ADR 0032). How to add one is in [`crates/nama-proto/README.md`](../crates/nama-proto/README.md).

Newest entry first. Each entry is a `## YYYY-MM-DD, #<issue>` heading, dated the day the change merges, followed by:

- each rule and file `buf breaking` reports;
- what changed in the schema;
- what a client or an Installed Plugin built before the change must do.

CI counts those headings, so a break passes only with one more than main has: editing an entry passes nothing, and deleting one is a change of its own.

## 2026-11-11, #624

- `FIELD_SAME_TYPE`, `nama/v1/browse.proto`.
- `ContinueWatchingEntry.episode` (3) is now a `ContinueWatchingEpisode`, which holds the `Episode` and the `Show` it is of. `ContinueWatchingEntry.show` (4) is removed and reserved.
- A client built before misreads an Episode entry, or fails to decode the response, and finds no Show; it must regenerate and read both from the Episode arm. No Installed Plugin sees Continue Watching.

## 2026-11-11, #625

- `ENUM_NO_DELETE` and `MESSAGE_NO_DELETE`, `nama/plugin/v1/manifest.proto`: `SettingKind` and `SettingValue` are deleted.
- `FIELD_SAME_TYPE`, `nama/plugin/v1/manifest.proto`: `SettingField.kind` is now a `nama.v1.SettingKind` and `SettingField.default_value` a `nama.v1.SettingValue`.

A Plugin's Manifest declares its settings with the setting kind and value an Admin is shown, so the schema has one of each. The two numbered `URL` and `CHOICE` the other way round, and the values' fields differ in name and number, so the Nama API version is now 0.2.0, and a Server refuses a Plugin built for 0.1, naming the version it wants, rather than reading its URL settings as choices.

An Installed Plugin built before must be rebuilt against `nama-pdk` with `nama-proto` 0.2.0, taking `SettingKind` and `SettingValue` from `nama_proto::nama::v1` and writing a default's text as `setting_value::Value::TextValue` (and `BoolValue` and `IntegerValue`). Clients of `nama.v1` need do nothing.
