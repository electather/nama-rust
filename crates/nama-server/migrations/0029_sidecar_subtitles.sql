-- The subtitle files a scan found beside each Version, and what their names say (owned as
-- `src/playback/AGENTS.md` says). Only their names and metadata are read, never the files. Rows
-- go with their Version.
CREATE TABLE sidecar_subtitles (
    version_id INTEGER NOT NULL REFERENCES library_files (id) ON DELETE CASCADE,
    path TEXT NOT NULL,
    size INTEGER NOT NULL,
    modified_ns INTEGER NOT NULL,
    -- As ffprobe would name it, from the extension, such as `subrip`.
    codec TEXT NOT NULL,
    -- As the name writes it, such as `en`, `eng` or `pt-BR`.
    language TEXT,
    title TEXT,
    is_default INTEGER NOT NULL CHECK (is_default IN (0, 1)),
    forced INTEGER NOT NULL CHECK (forced IN (0, 1)),
    sdh INTEGER NOT NULL CHECK (sdh IN (0, 1)),
    PRIMARY KEY (version_id, path)
) STRICT;
