-- Each Version ffprobe couldn't read: why, and the probe and failure versions, size and modified
-- time it failed at (owned as the crate's `AGENTS.md` says). A lasting failure makes the probe pass
-- after each scan skip it until one of those changes; a passing one, such as a timeout, is tried
-- again next pass, up to a few times (`PROBE_ATTEMPTS`). A Version has a row here or in
-- `version_probes`, never both.
CREATE TABLE probe_failures (
    version_id INTEGER PRIMARY KEY REFERENCES library_files (id) ON DELETE CASCADE,
    -- `failed` and `unreadable_output` last; `timed_out` and `not_run` pass.
    reason TEXT NOT NULL CHECK (reason IN ('failed', 'unreadable_output', 'timed_out', 'not_run')),
    probe_version INTEGER NOT NULL,
    -- Like `probe_version`, but bumped to retry only failed files (`PROBE_FAILURE_VERSION`).
    failure_version INTEGER NOT NULL,
    size INTEGER NOT NULL,
    modified_ns INTEGER NOT NULL,
    -- How many times in a row it failed at these versions, size and modified time.
    attempts INTEGER NOT NULL
) STRICT;
