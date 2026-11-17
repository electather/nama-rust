-- When each Watch State last became played, in seconds since the Unix epoch, which picks a Show's
-- next Episode (#915); `updated_at` won't do, since rewatching a played Episode moves it. NULL
-- while it's unplayed. One played before this migration is taken as played when it last changed,
-- the nearest time there is.
ALTER TABLE watch_states ADD COLUMN played_at INTEGER;
UPDATE watch_states SET played_at = updated_at WHERE played;
