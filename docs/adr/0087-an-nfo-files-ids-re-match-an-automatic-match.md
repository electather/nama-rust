# An `.nfo` file's IDs re-Match an automatic Match

People moving from Kodi, Jellyfin or Emby bring `.nfo` files that name each title's External IDs (#1418). The Kernel reads only those IDs during a Scan, as ID Hints, and none of the rest of the file. When an `.nfo` file appears or changes, its IDs replace the name's ID Hints as a set, and an Item whose automatic Match disagrees with them is Matched again. A Pinned Match is never touched. ADR 0023 leaves automatic Matches alone when the parser changes, because that change is ours and could switch a title people are watching. An `.nfo` edit is the Admin's own act, like renaming the file, so it is allowed to switch the title.

## Considered Options

- **NFO as a title source, filling metadata, as Silo does**: rejected. It means a second metadata format and a file-read Permission for Plugins, while IDs alone fix the wrong Matches.
- **Stored IDs win except on a manual refresh, as in Silo**: rejected. Nama has no manual refresh, so a corrected `.nfo` would never take effect.
- **The name's ID Hints win over the `.nfo` file's**: rejected. A name's ID Hint is often left by a downloader, while an `.nfo` file is usually curated. The name's ID Hints are tried only when the title sources reject every `.nfo` ID.

## Consequences

- An `.nfo` file is stored with its own size and modified time, so editing it alone is noticed on the next Scan.
- An `.nfo` file that won't parse counts as none, with a warning, so an Admin can tell it apart from a missing one.
