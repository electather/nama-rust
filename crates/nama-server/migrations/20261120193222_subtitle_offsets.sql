-- Each User's Subtitle Offsets: how many milliseconds later than its file times it they show a
-- subtitle Track of a Version, negative for earlier (ADR 0070). A Track with no row shows as timed.
-- `track` is a sidecar's or downloaded subtitle's `path`, so one added later, which can shift
-- places, never takes another's offset, or else the decimal place in the Version's Tracks of one
-- inside its file. Rows go with their User or Version.
CREATE TABLE subtitle_offsets (
    user_id INTEGER NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    version_id INTEGER NOT NULL REFERENCES library_files (id) ON DELETE CASCADE,
    track TEXT NOT NULL,
    offset_ms INTEGER NOT NULL,
    PRIMARY KEY (user_id, version_id, track)
) STRICT;
