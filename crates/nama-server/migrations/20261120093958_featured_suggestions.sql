-- What each Plugin last suggested for one User's Featured through `user_featured` (ADR 0072), and
-- when to ask it again. A row is inserted, keeping no suggestions, as its first refresh starts.
CREATE TABLE featured_suggestions (
    plugin_id TEXT NOT NULL,
    user_id INTEGER NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    -- A JSON array of the titles suggested, each `[["movie", "tmdb-603"], 0.5]`, with its weight.
    suggestions TEXT NOT NULL,
    stale_at INTEGER NOT NULL,
    PRIMARY KEY (plugin_id, user_id)
) STRICT;

CREATE INDEX featured_suggestions_user ON featured_suggestions (user_id);

-- Suggestions are built from the User's Personal settings, so saving or deleting them deletes the
-- User's suggestions from that Plugin, which their next visit to home asks for again, and drops
-- an answer to a refresh started with the old settings. Saving replaces every row
-- (`src/users/personal_settings.rs`), so inserts and deletes are every change.
CREATE TRIGGER featured_suggestions_personal_settings_added AFTER INSERT ON personal_settings
BEGIN
    DELETE FROM featured_suggestions WHERE plugin_id = NEW.plugin_id AND user_id = NEW.user_id;
END;

CREATE TRIGGER featured_suggestions_personal_settings_removed AFTER DELETE ON personal_settings
BEGIN
    DELETE FROM featured_suggestions WHERE plugin_id = OLD.plugin_id AND user_id = OLD.user_id;
END;
