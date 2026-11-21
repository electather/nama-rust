-- Who started each Job Run (ADR 0079): the Admin's name as it was then, not their id, so a Run
-- still names an Admin since deleted; NULL for a Run its schedule started.
ALTER TABLE job_runs ADD COLUMN started_by TEXT;
