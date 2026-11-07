-- Whether each Plugin is enabled, and its settings, by Plugin id. A Built-in Plugin with no row is
-- enabled with no settings. Settings may hold a secret, such as an API key, stored as written: the
-- Plugin needs the value itself to send it (the crate's `AGENTS.md`).
CREATE TABLE plugin_settings (
    plugin_id TEXT PRIMARY KEY NOT NULL,
    enabled INTEGER NOT NULL CHECK (enabled IN (0, 1)),
    -- A JSON object from each setting's key to its value: text, a whole number or a boolean.
    settings TEXT NOT NULL CHECK (json_type(settings) = 'object')
) STRICT;
