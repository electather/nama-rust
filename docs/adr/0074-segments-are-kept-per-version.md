# Segments are kept per Version

The player skips an Episode's Intro and Recap and starts the Next Episode card when the Credits start (#1213), so it needs to know where each Segment is. Per-title data keys on External ID (`crates/nama-server/AGENTS.md`), and Segments were listed there among the data to come. They key on the Version instead, as downloaded subtitles do (ADR 0067): a Segment is a stretch of time, and times shift between cuts and encodes of the same Episode, such as one with a cold open trimmed or one taken from a different broadcast. A Version's Chapters are per Version for the same reason, and because the file itself lists them.

An outside service answers by External ID, season, episode and the file's duration. The Kernel stores that answer against the Version it asked for, with the source that gave it, so a second Version of the same Episode is asked about separately and may get different times.

Each Version keeps at most one Segment of each kind. The Kernel drops a Segment that ends before it starts or starts after the Version's duration, and cuts an end past the duration back to it; a Segment with no end runs to the end, as Credits often do. A Version's Segments are deleted with it.

## Considered Options

- **Key on External ID**, as the rule said: rejected. A second Version of the same Episode would skip to the wrong time, and the Kernel couldn't tell which file the times were measured on.
- **Key on External ID with an offset per Version**: rejected. An offset fixes a shifted start but not a trimmed middle, and nothing measures it.

## Consequences

- A rename or remount that makes a new Version loses its Segments until they're asked for again, which happens in the background (ADR 0075).
- `crates/nama-server/AGENTS.md` names Segments with downloaded subtitles as the data that keys on the Version.
