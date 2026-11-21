-- Where the Server writes its Backups, or '' for `backups` in the data directory, and how many of
-- them it keeps, or NULL for the default (#1224).
ALTER TABLE server_settings ADD COLUMN backup_folder TEXT NOT NULL DEFAULT '';
ALTER TABLE server_settings ADD COLUMN backups_kept INTEGER;
