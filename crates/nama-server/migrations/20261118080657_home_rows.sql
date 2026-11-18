-- The Home rows each Plugin gave at its last refresh (ADR 0058), in its order. A refresh replaces
-- a Plugin's rows in one write, and those of a Plugin no longer serving them are deleted.
CREATE TABLE home_rows (
    plugin_id TEXT NOT NULL,
    -- Its place among the Plugin's rows, from 0.
    position INTEGER NOT NULL,
    key TEXT NOT NULL,
    -- At most 40 characters: the Server cuts it.
    heading TEXT NOT NULL,
    -- A JSON array of the row's titles in its order, each `["movie", "tmdb-603"]` or `["show", ...]`.
    titles TEXT NOT NULL,
    PRIMARY KEY (plugin_id, position),
    UNIQUE (plugin_id, key)
) STRICT;

-- When each Plugin's Home rows next go stale: when the first of them does, or for one that gave
-- none, 15 minutes after. A Plugin with no row here has never been refreshed.
CREATE TABLE home_row_refreshes (
    plugin_id TEXT PRIMARY KEY,
    stale_at INTEGER NOT NULL
) STRICT;
