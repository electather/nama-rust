-- The Chapters probing read from each Version's file, in the order the file lists them. Keyed on
-- the probe they were read in, like `tracks`, so they go when the file is probed again or fails to
-- be, and with the Version.
CREATE TABLE chapters (
    version_id INTEGER NOT NULL REFERENCES version_probes (version_id) ON DELETE CASCADE,
    position INTEGER NOT NULL,
    -- From the file's start.
    start_ms INTEGER NOT NULL,
    -- Empty when the file names none.
    title TEXT NOT NULL,
    PRIMARY KEY (version_id, position)
) STRICT;
