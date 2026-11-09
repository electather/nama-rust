-- A copy of the Quality a Followed Show's Season Requests take, as `requests` keeps one (ADR 0025),
-- NULL if the Shows Fulfiller had none when it opened.
ALTER TABLE followed_shows ADD COLUMN quality_id INTEGER;
ALTER TABLE followed_shows ADD COLUMN quality_name TEXT;
ALTER TABLE followed_shows ADD COLUMN quality_profile_id TEXT;
