-- Hiding from Continue Watching is gone (#1331), and the hides it stored with it.
ALTER TABLE watch_states DROP COLUMN hidden;
