-- One row per video file per Library. `size` and `modified_ns` (nanoseconds since the Unix
-- epoch) are what a rescan compares to skip a file that hasn't changed.
CREATE TABLE library_files (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    library_id INTEGER NOT NULL REFERENCES libraries (id) ON DELETE CASCADE,
    -- Absolute path, under one of the Library's folders.
    path TEXT NOT NULL,
    size INTEGER NOT NULL,
    modified_ns INTEGER NOT NULL,
    -- The version of the name parser that read each file (ADR 0023). A scan parses a file again when
    -- it's older than the current one.
    parser_version INTEGER NOT NULL DEFAULT 1,
    UNIQUE (library_id, path)

) STRICT;
