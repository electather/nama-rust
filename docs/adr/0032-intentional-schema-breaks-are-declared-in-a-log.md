# Intentional schema breaks are declared in a log

ADR 0007 has CI run `buf breaking` against main on every change, so no change breaks the schema by accident. Some changes should break it: a field whose shape only a comment states, such as two fields of which one is set, can't become a oneof without a break. Such a change adds an entry to [`proto/BREAKING.md`](../../proto/BREAKING.md), newest first, saying the date, the issue, each rule and file `buf breaking` reports, what changed, and what a client or an Installed Plugin built before must do. When the check fails and the change adds an entry, CI prints the breaks and passes. Otherwise a break fails as before. `proto/buf.yaml`'s rules stay as they are.

## Considered Options

- **A per-change exception in `proto/buf.yaml`**: rejected. Each break needs a commit to add the exception and another to take it out, and an exception left behind would let later breaks of the same rule and file through.
- **Never break the schema**: rejected. A shape that a comment states can then only be fixed under new field names, leaving the old ones behind for good.
- **Turn `buf breaking` off until the first release**: rejected. Nothing would show which changes break clients and Installed Plugins, and turning it back on is easily forgotten.

## Consequences

- CI counts entries rather than comparing them, so editing or deleting an entry passes nothing, and an entry can't be added in the change that deletes another.
- CI doesn't check that an entry names every break the check reports; review does.
