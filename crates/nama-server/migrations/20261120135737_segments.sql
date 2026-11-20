-- The ids of the Plugins whose Segment sources a Library asks, in the order it asks them,
-- comma-separated as `recognisers` is. Empty asks none, leaving the Chapter names alone (ADR 0075).
ALTER TABLE libraries ADD COLUMN segment_sources TEXT NOT NULL DEFAULT '';

-- When a Version still missing a kind of Segment is next asked about, in milliseconds since the
-- epoch. Unset until it is first asked after this probe, so a Version probed again is asked again.
ALTER TABLE version_probes ADD COLUMN segments_due_ms INTEGER;

-- A Version's Segments, at most one of each kind. Keyed on the Version, not the title, since their
-- times fit one file (ADR 0074).
CREATE TABLE segments (
    version_id INTEGER NOT NULL REFERENCES library_files (id) ON DELETE CASCADE,
    kind TEXT NOT NULL CHECK (kind IN ('intro', 'recap', 'credits')),
    -- From the file's start, as Chapters are.
    start_ms INTEGER NOT NULL,
    -- Unset runs to the end.
    end_ms INTEGER,
    -- The id of the Plugin whose Segment source gave it; unset when the Kernel read it from the
    -- Version's Chapter names.
    source TEXT,
    PRIMARY KEY (version_id, kind)
) STRICT;
