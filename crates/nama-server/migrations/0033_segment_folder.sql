-- The folder playback writes segment files to, an absolute path, or '' for data_dir/transcodes.
ALTER TABLE server_settings ADD COLUMN segment_folder TEXT NOT NULL DEFAULT '';
