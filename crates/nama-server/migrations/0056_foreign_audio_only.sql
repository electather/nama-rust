-- The User's own Subtitle Preference for full subtitles can ask for them only over audio in
-- another language (ADR 0035). Only that row is ever 1: a Show's or Movie's has no such condition.
ALTER TABLE track_preferences
    ADD COLUMN foreign_audio_only INTEGER NOT NULL DEFAULT 0 CHECK (foreign_audio_only IN (0, 1));
