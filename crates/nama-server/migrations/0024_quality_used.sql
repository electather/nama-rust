-- The Quality a Request's service downloads it at, as its hand-off reported it (ADR 0025). NULL
-- until handed off, and when it took no Quality. `quality_used_id` is 0 when no Quality on the
-- list had the profile, and the name is then the service's own: set in the service.
ALTER TABLE requests ADD COLUMN quality_used_id INTEGER;
ALTER TABLE requests ADD COLUMN quality_used_name TEXT;
ALTER TABLE requests ADD COLUMN quality_used_profile_id TEXT;
