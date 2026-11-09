# Each file remembers which version of the name parser read it

A rescan skips a file whose size and modified time haven't changed, and an Unmatched decision is kept until its file changes. So when the name parser learns something new, such as an ID Hint spelling or a country tag, files already in a Library never benefit. Each stored file therefore records the version of the name parser that read it. A scan parses a file again when that version is older than the current one, and clears its stored automatic Unmatched decision so Matching tries it again. Automatic Matches and pins are left alone: re-deciding a Match could change a title people are already watching, and a pin is an admin's choice. Any change that could turn an Unmatched Item into a Match, in how names are parsed or how Matching decides, bumps the version.

## Considered Options

- **A one-off migration per parser change that resets every file's size and modified time**: rejected. Each change needs its own hand-written migration, and forgetting it fails silently, with old files keeping their old names.

## Consequences

- When a re-parse changes a name's pin key (its title, year or ID Hint), the pin is moved to the new key, so a pinned Match stays pinned.
- Bumping the version costs one parse of every file on the next scan, plus a metadata search for each item that was Unmatched.
