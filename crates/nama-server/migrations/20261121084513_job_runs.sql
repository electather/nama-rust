-- Each Job Run (ADR 0079), written when it starts and closed when it ends. Keyed by the Plugin and
-- Job ids, not a Plugin row, so a disabled Plugin's history waits for it to be enabled again.
CREATE TABLE job_runs (
    id INTEGER PRIMARY KEY,
    plugin TEXT NOT NULL,
    job TEXT NOT NULL,
    outcome TEXT NOT NULL CHECK (
        outcome IN ('running', 'succeeded', 'failed', 'timed_out', 'interrupted')
    ),
    started_ms INTEGER NOT NULL,
    ended_ms INTEGER,
    -- What the Plugin said, for a failed Run.
    error TEXT
) STRICT;

CREATE INDEX job_runs_by_job ON job_runs (plugin, job, id);
