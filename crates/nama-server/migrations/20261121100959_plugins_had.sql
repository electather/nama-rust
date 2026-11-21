-- When the Server last had each Plugin, enabled or not, so Upkeep can delete what one it stopped
-- having left behind (#1452). Every Plugin something is kept for counts as had now, so an upgrade
-- deletes nothing before a Plugin could have been gone that long.
CREATE TABLE plugins_had (
    plugin_id TEXT PRIMARY KEY NOT NULL,
    had_at INTEGER NOT NULL
) STRICT;

INSERT INTO plugins_had (plugin_id, had_at)
SELECT plugin_id, unixepoch() FROM plugin_settings
UNION SELECT plugin_id, unixepoch() FROM plugin_store
UNION SELECT plugin_id, unixepoch() FROM personal_settings
UNION SELECT plugin_id, unixepoch() FROM plugin_connections;
