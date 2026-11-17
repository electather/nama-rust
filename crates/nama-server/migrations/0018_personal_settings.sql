-- Personal settings: what each User sets for themself in a Plugin whose Manifest declares them,
-- such as their ntfy topic or Discord webhook (ADR 0046). Only `src/personal_settings.rs` reads
-- it, and hands a Plugin the values of the User a call is for. Deleting the User deletes theirs.
CREATE TABLE personal_settings (
    user_id INTEGER NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    plugin_id TEXT NOT NULL,
    key TEXT NOT NULL,
    -- As the User wrote it, since the Plugin sends it as it is. Never logged.
    value TEXT NOT NULL,
    -- Whether the Manifest declared it secret: its value never goes back to a client.
    secret INTEGER NOT NULL,
    PRIMARY KEY (user_id, plugin_id, key)
) STRICT;
