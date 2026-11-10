-- Each Version ffprobe couldn't read, and the probe version, size and modified time it failed at,
-- so the probe pass after each scan skips it until one of them changes (owned as the crate's
-- `AGENTS.md` says). A Version has a row here or in `version_probes`, never both.
CREATE TABLE probe_failures (
    version_id INTEGER PRIMARY KEY REFERENCES library_files (id) ON DELETE CASCADE,
    probe_version INTEGER NOT NULL,
    size INTEGER NOT NULL,
    modified_ns INTEGER NOT NULL
) STRICT;
