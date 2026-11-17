-- One row per probed Version. A Version without one hasn't been probed yet.
CREATE TABLE version_probes (
    version_id INTEGER PRIMARY KEY REFERENCES library_files (id) ON DELETE CASCADE,
    -- The version of the probe code that read it, like `library_files.parser_version` (ADR 0023).
    probe_version INTEGER NOT NULL,
    -- As ffprobe names it, such as `matroska,webm`.
    container TEXT NOT NULL,
    duration_ms INTEGER,
    -- Overall, in bits per second.
    bitrate INTEGER,
    -- The size and modified time of the file each Version was probed at, so a file changed since is
    -- probed again. The default, -1, is no file's size, so a row left at it is probed again too.
    size INTEGER NOT NULL DEFAULT -1,
    modified_ns INTEGER NOT NULL DEFAULT -1,
    -- When each keyframe of a Version's first video falls, in milliseconds, as a JSON array: listed
    -- the first time the Version is Remuxed and kept for the next. It goes with its row, so a file
    -- probed again has them listed again.
    keyframes_ms TEXT,
    -- The fonts a Version's file carries, as a JSON array of each one's place among the file's
    -- attachments, which is how ffmpeg picks it out.
    font_attachments TEXT NOT NULL DEFAULT '[]',
    -- The `PROBE_FAILURE_VERSION` at which listing a Version's keyframes failed or timed out, or NULL
    -- if it hasn't. It goes with its row, as `keyframes_ms` does, so a file that changes and is
    -- probed again has them listed again, and listing isn't tried again until then or a bump (#850).
    keyframes_failure_version INTEGER,
    -- When a Version's file starts, in milliseconds, as Probing read it with `keyframes_ms`: a Remux
    -- cuts copied audio by it in a file that starts late, such as an MPEG-TS (#944). Keyframes kept
    -- without it are listed again.
    keyframes_start_ms INTEGER
) STRICT;
