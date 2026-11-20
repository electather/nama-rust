-- A Plugin granted Read Watch States may build a User's own Home rows and Featured suggestions
-- from their Watch State, such as leaving out what they started (#1326). So starting a title,
-- marking it played or marking it unplayed makes that User's rows and suggestions from each such
-- Plugin stale, as saving their Personal settings does (the triggers on `personal_settings`): their
-- next visit to home asks the Plugin again, and an answer to a refresh started before is dropped. A
-- position the player reports while playing changes neither, so playback's frequent writes cost
-- nothing.

-- The Plugins an Admin granted Read Watch States, which `plugin_settings` keeps by its name.
CREATE VIEW plugins_reading_watch_states AS
SELECT plugin_id FROM plugin_settings, json_each(granted_permissions)
WHERE json_each.value = 'PERMISSION_READ_WATCH_STATES';

CREATE TRIGGER home_rows_watch_state_started AFTER INSERT ON watch_states
BEGIN
    DELETE FROM home_row_refreshes WHERE user_id = NEW.user_id
        AND plugin_id IN (SELECT plugin_id FROM plugins_reading_watch_states);
    DELETE FROM featured_suggestions WHERE user_id = NEW.user_id
        AND plugin_id IN (SELECT plugin_id FROM plugins_reading_watch_states);
END;

CREATE TRIGGER home_rows_watch_state_played AFTER UPDATE OF played ON watch_states
WHEN OLD.played IS NOT NEW.played
BEGIN
    DELETE FROM home_row_refreshes WHERE user_id = NEW.user_id
        AND plugin_id IN (SELECT plugin_id FROM plugins_reading_watch_states);
    DELETE FROM featured_suggestions WHERE user_id = NEW.user_id
        AND plugin_id IN (SELECT plugin_id FROM plugins_reading_watch_states);
END;

-- Marking a title unplayed deletes its Watch State.
CREATE TRIGGER home_rows_watch_state_unplayed AFTER DELETE ON watch_states
BEGIN
    DELETE FROM home_row_refreshes WHERE user_id = OLD.user_id
        AND plugin_id IN (SELECT plugin_id FROM plugins_reading_watch_states);
    DELETE FROM featured_suggestions WHERE user_id = OLD.user_id
        AND plugin_id IN (SELECT plugin_id FROM plugins_reading_watch_states);
END;
