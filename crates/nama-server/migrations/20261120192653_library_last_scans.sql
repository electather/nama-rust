-- A Library's last Scan that finished storing, each one replacing the last; no history is kept.
-- When it finished, in seconds since the epoch; unset before the first, which leaves the counts
-- meaningless.
ALTER TABLE libraries ADD COLUMN last_scan_at INTEGER;
ALTER TABLE libraries ADD COLUMN last_scan_added INTEGER NOT NULL DEFAULT 0;
ALTER TABLE libraries ADD COLUMN last_scan_updated INTEGER NOT NULL DEFAULT 0;
ALTER TABLE libraries ADD COLUMN last_scan_removed INTEGER NOT NULL DEFAULT 0;
ALTER TABLE libraries ADD COLUMN last_scan_unchanged INTEGER NOT NULL DEFAULT 0;
