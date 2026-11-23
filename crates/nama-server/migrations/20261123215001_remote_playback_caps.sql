-- The most a User's Device outside the local network is sent, whatever Playback Limit it picks,
-- by the name `server_settings` stores a Playback Limit under ("720p"). NULL is no cap. The
-- default is the one each new User gets.
ALTER TABLE users ADD COLUMN remote_playback_limit TEXT;
ALTER TABLE default_permissions ADD COLUMN remote_playback_limit TEXT;
