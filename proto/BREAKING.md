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
- A client built before reads an Episode entry's field 3 as an `Episode` with wrong fields and finds no Show, so it must regenerate and read both from the Episode arm. No Installed Plugin sees Continue Watching.
