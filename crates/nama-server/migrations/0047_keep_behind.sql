-- How many minutes behind playback a running Remux or Transcode keeps its files, or NULL for the
-- default.
ALTER TABLE server_settings ADD COLUMN keep_behind_minutes INTEGER;
