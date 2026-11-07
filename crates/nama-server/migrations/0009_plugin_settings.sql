-- Whether each Plugin is enabled, and its settings, by Plugin id. A Built-in Plugin with no row is
-- enabled with no settings. Secrets here are stored as written (the crate's `AGENTS.md`).
CREATE TABLE plugin_settings (
    plugin_id TEXT PRIMARY KEY NOT NULL,
    enabled INTEGER NOT NULL CHECK (enabled IN (0, 1)),
    -- A JSON object from each setting's key to its value: text, a whole number or a boolean.
    settings TEXT NOT NULL CHECK (json_type(settings) = 'object')
) STRICT;
