-- Home rows gain a scope: a Plugin's shared rows, as before, or one User's own, deleted with them
-- (ADR 0058). The rows are only what Plugins last gave, so both tables are recreated empty, and
-- the next visit to home refreshes them.
DROP TABLE home_rows;
DROP TABLE home_row_refreshes;

-- The Home rows each Plugin gave at its last refresh of a scope, in its order. A refresh replaces
-- a Plugin's rows in that scope in one write, and those of a Plugin no longer serving them are
-- deleted.
CREATE TABLE home_rows (
    plugin_id TEXT NOT NULL,
    -- The User whose own rows these are, or NULL for the rows every User sees.
    user_id INTEGER REFERENCES users (id) ON DELETE CASCADE,
    -- Its place among the Plugin's rows in its scope, from 0.
    position INTEGER NOT NULL,
    key TEXT NOT NULL,
    -- At most 40 characters: the Server cuts it.
    heading TEXT NOT NULL,
    -- A JSON array of the row's titles in its order, each `["movie", "tmdb-603"]` or `["show", ...]`.
    titles TEXT NOT NULL
) STRICT;

-- A UNIQUE constraint takes two NULLs as different, so the shared scope is keyed as 0, which no
-- User's id is.
CREATE UNIQUE INDEX home_rows_place ON home_rows (plugin_id, coalesce(user_id, 0), position);
CREATE UNIQUE INDEX home_rows_key ON home_rows (plugin_id, coalesce(user_id, 0), key);
CREATE INDEX home_rows_user ON home_rows (user_id);

-- When each Plugin's Home rows in a scope next go stale: when the first of them does, or for a
-- scope that got none, 15 minutes after. A scope with no row here has never been refreshed.
CREATE TABLE home_row_refreshes (
    plugin_id TEXT NOT NULL,
    -- As `home_rows.user_id`.
    user_id INTEGER REFERENCES users (id) ON DELETE CASCADE,
    stale_at INTEGER NOT NULL
) STRICT;

CREATE UNIQUE INDEX home_row_refreshes_scope ON home_row_refreshes (plugin_id, coalesce(user_id, 0));
CREATE INDEX home_row_refreshes_user ON home_row_refreshes (user_id);

-- A User's own rows are built from their Personal settings, so saving or deleting them makes the
-- User's rows from that Plugin stale, refreshed on their next visit to home rather than at their
-- time to live, up to a week on. Saving replaces every row (`src/users/personal_settings.rs`), so
-- inserts and deletes are every change.
CREATE TRIGGER home_row_refreshes_personal_settings_added AFTER INSERT ON personal_settings
BEGIN
    DELETE FROM home_row_refreshes WHERE plugin_id = NEW.plugin_id AND user_id = NEW.user_id;
END;

CREATE TRIGGER home_row_refreshes_personal_settings_removed AFTER DELETE ON personal_settings
BEGIN
    DELETE FROM home_row_refreshes WHERE plugin_id = OLD.plugin_id AND user_id = OLD.user_id;
END;
