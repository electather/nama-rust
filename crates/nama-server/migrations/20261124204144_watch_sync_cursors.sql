-- Where each Watch sync Plugin's next import of a User's Watch State continues from: the cursor
-- its last import returned, opaque to the Server. Deleting the User deletes theirs; a removed
-- Plugin's go with its Personal settings (`LEFTOVERS`).
CREATE TABLE watch_sync_cursors (
    user_id INTEGER NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    plugin_id TEXT NOT NULL,
    cursor TEXT NOT NULL,
    PRIMARY KEY (user_id, plugin_id)
) STRICT;
