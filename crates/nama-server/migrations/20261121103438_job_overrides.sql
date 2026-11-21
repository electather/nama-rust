-- What an Admin changed of a Job (ADR 0079): turned it off, or gave it a schedule in place of its
-- Manifest's. A Job nobody changed has no row, so a Plugin update's new schedule reaches it. Keyed
-- by the Plugin and Job ids, as job_runs is, so a disabled Plugin's overrides wait for it.
CREATE TABLE job_overrides (
    plugin TEXT NOT NULL,
    job TEXT NOT NULL,
    enabled INTEGER NOT NULL CHECK (enabled IN (0, 1)),
    -- NULL: the Manifest's.
    schedule TEXT,
    PRIMARY KEY (plugin, job),
    CHECK (enabled = 0 OR schedule IS NOT NULL)
) STRICT;
