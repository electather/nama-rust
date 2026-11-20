-- When to ask each Plugin again for one User's Featured suggestions, kept apart from the suggestions
-- as `home_row_refreshes` is from `home_rows`: what makes the suggestions stale deletes only this
-- row, so they stay listed until the Plugin answers again (#1334). A row is put here as each
-- refresh starts, and its answer is stored only while the row is still here, so an answer to a
-- refresh started before the change is dropped.
CREATE TABLE featured_refreshes (
    plugin_id TEXT NOT NULL,
    user_id INTEGER NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    stale_at INTEGER NOT NULL,
    PRIMARY KEY (plugin_id, user_id)
) STRICT;

CREATE INDEX featured_refreshes_user ON featured_refreshes (user_id);

INSERT INTO featured_refreshes (plugin_id, user_id, stale_at)
SELECT plugin_id, user_id, stale_at FROM featured_suggestions;

ALTER TABLE featured_suggestions DROP COLUMN stale_at;

DROP TRIGGER featured_suggestions_personal_settings_added;
DROP TRIGGER featured_suggestions_personal_settings_removed;
DROP TRIGGER home_rows_watch_state_started;
DROP TRIGGER home_rows_watch_state_played;
DROP TRIGGER home_rows_watch_state_unplayed;

CREATE TRIGGER featured_refreshes_personal_settings_added AFTER INSERT ON personal_settings
BEGIN
    DELETE FROM featured_refreshes WHERE plugin_id = NEW.plugin_id AND user_id = NEW.user_id;
END;

CREATE TRIGGER featured_refreshes_personal_settings_removed AFTER DELETE ON personal_settings
BEGIN
    DELETE FROM featured_refreshes WHERE plugin_id = OLD.plugin_id AND user_id = OLD.user_id;
END;

CREATE TRIGGER home_rows_watch_state_started AFTER INSERT ON watch_states
BEGIN
    DELETE FROM home_row_refreshes WHERE user_id = NEW.user_id
        AND plugin_id IN (SELECT plugin_id FROM plugins_reading_watch_states);
    DELETE FROM featured_refreshes WHERE user_id = NEW.user_id
        AND plugin_id IN (SELECT plugin_id FROM plugins_reading_watch_states);
END;

CREATE TRIGGER home_rows_watch_state_played AFTER UPDATE OF played ON watch_states
WHEN OLD.played IS NOT NEW.played
BEGIN
    DELETE FROM home_row_refreshes WHERE user_id = NEW.user_id
        AND plugin_id IN (SELECT plugin_id FROM plugins_reading_watch_states);
    DELETE FROM featured_refreshes WHERE user_id = NEW.user_id
        AND plugin_id IN (SELECT plugin_id FROM plugins_reading_watch_states);
END;

-- Marking a title unplayed deletes its Watch State.
CREATE TRIGGER home_rows_watch_state_unplayed AFTER DELETE ON watch_states
BEGIN
    DELETE FROM home_row_refreshes WHERE user_id = OLD.user_id
        AND plugin_id IN (SELECT plugin_id FROM plugins_reading_watch_states);
    DELETE FROM featured_refreshes WHERE user_id = OLD.user_id
        AND plugin_id IN (SELECT plugin_id FROM plugins_reading_watch_states);
END;
