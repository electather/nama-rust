-- Whether a Library makes Preview frames for its Versions: on unless an Admin turns it off, so
-- existing Libraries make them too (ADR 0076).
ALTER TABLE libraries ADD COLUMN preview_frames INTEGER NOT NULL DEFAULT 1
    CHECK (preview_frames IN (0, 1));

-- The Preview frames made of each Version's file, kept in the blob store under `preview-frames/`,
-- or that making them failed (ADR 0076; owned as `src/playback/AGENTS.md` says). Either is of the
-- file at `size` and `modified_ns`, so a changed file is made again. Rows go with their Version.
CREATE TABLE preview_frames (
    version_id INTEGER PRIMARY KEY REFERENCES library_files (id) ON DELETE CASCADE,
    size INTEGER NOT NULL,
    modified_ns INTEGER NOT NULL,
    -- The manifest, every column NULL when making them failed.
    interval_ms INTEGER,
    width INTEGER,
    height INTEGER,
    sheet_columns INTEGER,
    sheet_rows INTEGER,
    frames INTEGER
) STRICT;
