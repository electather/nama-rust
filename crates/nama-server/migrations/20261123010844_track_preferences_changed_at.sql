-- When the User last changed each Track Preference, in milliseconds since the Unix epoch, so a
-- change pulled from their other Servers is taken only where it is newer (ADR 0092). 0 for one
-- kept before Preferences synced, which any pulled change is newer than.
ALTER TABLE track_preferences ADD COLUMN changed_at INTEGER NOT NULL DEFAULT 0;
