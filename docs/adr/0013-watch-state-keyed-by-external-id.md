# Watch State is keyed by external ID on the Server too, so Unmatched Items have none

A Server stores each User's Watch State against the Movie's or Episode's external metadata IDs, the same key the Hub uses (ADR 0008), not against an Item or file id. Items are replaced whenever a file changes, and a Movie can have several Versions. Keying on what the file *is*, not which file it is, means progress survives rescans, renames and a new Version, and syncing with the Hub needs no translation. The cost is that an Unmatched Item has no Watch State: it can be played, but nothing is remembered until it is Matched.

## Consequences

- A file holding several Episodes plays as one file, and its position is split across those Episodes.
- Setting or clearing a Match by hand moves which Watch State an Item shows, because the key changes with the Match.
