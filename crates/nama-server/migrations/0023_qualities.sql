-- Each Fulfiller's Qualities: the service quality profiles an admin offers, under friendly names
-- (ADR 0025). Keyed by Plugin, as Plugins have no storage of their own.
CREATE TABLE qualities (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    plugin_id TEXT NOT NULL,
    -- Where the admin listed it, from 0.
    position INTEGER NOT NULL,
    -- A choice the Plugin's settings check offers for its `quality_profile` setting.
    profile_id TEXT NOT NULL,
    name TEXT NOT NULL,
    description TEXT NOT NULL,
    is_default INTEGER NOT NULL CHECK (is_default IN (0, 1)),
    UNIQUE (plugin_id, profile_id),
    UNIQUE (plugin_id, name COLLATE NOCASE)
) STRICT;

CREATE UNIQUE INDEX one_default_quality_per_plugin ON qualities (plugin_id) WHERE is_default;
