-- Plugin Connections: a User's identity at an outside service, such as their Telegram chat id, that
-- a Plugin acts on for them (ADR 0021). Plugins have no storage, so the Kernel keeps it for the
-- Plugin `plugin_id`, which alone reads it. Deleting the User deletes theirs.
CREATE TABLE plugin_connections (
    user_id INTEGER NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    plugin_id TEXT NOT NULL,
    -- Opaque to the Kernel, and never sent to a client.
    connection TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    PRIMARY KEY (user_id, plugin_id)
) STRICT;
