-- The Playback Limit for remote playback, by its name, or '' for Original.
ALTER TABLE server_settings ADD COLUMN remote_playback_limit TEXT NOT NULL DEFAULT ''
    CHECK (remote_playback_limit IN ('', '1080p', '720p', '480p'));
