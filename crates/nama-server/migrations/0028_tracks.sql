-- What ffprobe found inside each Version: the facts of the whole file, and its Tracks (owned as
-- the crate's `AGENTS.md` says). Rows go with their file.

-- One row per probed Version. A Version without one hasn't been probed yet.
CREATE TABLE version_probes (
    version_id INTEGER PRIMARY KEY REFERENCES library_files (id) ON DELETE CASCADE,
    -- The version of the probe code that read it, like `library_files.parser_version` (ADR 0023).
    probe_version INTEGER NOT NULL,
    -- As ffprobe names it, such as `matroska,webm`.
    container TEXT NOT NULL,
    duration_ms INTEGER,
    -- Overall, in bits per second.
    bitrate INTEGER
) STRICT;

-- One row per video, audio or subtitle stream, in the order the file holds them.
CREATE TABLE tracks (
    version_id INTEGER NOT NULL REFERENCES version_probes (version_id) ON DELETE CASCADE,
    position INTEGER NOT NULL,
    kind TEXT NOT NULL CHECK (kind IN ('video', 'audio', 'subtitle')),
    codec TEXT NOT NULL,
    -- As the file tags it, usually ISO 639-2, such as `eng`.
    language TEXT,
    title TEXT,
    is_default INTEGER NOT NULL,
    forced INTEGER NOT NULL,
    width INTEGER CHECK ((kind = 'video') OR width IS NULL),
    height INTEGER CHECK ((kind = 'video') OR height IS NULL),
    frame_rate REAL CHECK ((kind = 'video') OR frame_rate IS NULL),
    channels INTEGER CHECK ((kind = 'audio') OR channels IS NULL),
    -- A bitmap subtitle, such as PGS, rather than text, such as SRT.
    image INTEGER CHECK ((kind = 'subtitle') = (image IS NOT NULL)),
    PRIMARY KEY (version_id, position)
) STRICT;
