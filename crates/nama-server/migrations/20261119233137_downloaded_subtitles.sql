-- The subtitles Users downloaded for each Version from a subtitle source, kept in the blob store
-- under `downloaded-subtitles/` (ADR 0067; owned as `src/playback/AGENTS.md` says). Listed after
-- the Version's sidecars, in the order they were downloaded. Rows go with their Version.
CREATE TABLE downloaded_subtitles (
    version_id INTEGER NOT NULL REFERENCES library_files (id) ON DELETE CASCADE,
    -- The file on local disk it plays from, as a sidecar does.
    path TEXT NOT NULL,
    size INTEGER NOT NULL,
    -- When it was downloaded, which its cached WebVTT is named by, as a sidecar's by its time.
    modified_ns INTEGER NOT NULL,
    -- As ffprobe would name it, from the format the source gave, such as `subrip`.
    codec TEXT NOT NULL,
    -- As the source tags it, a BCP 47 tag such as `en` or `pt-BR`.
    language TEXT NOT NULL,
    -- Its name at the source, such as its release name.
    title TEXT NOT NULL,
    sdh INTEGER NOT NULL CHECK (sdh IN (0, 1)),
    PRIMARY KEY (version_id, path)
) STRICT;
