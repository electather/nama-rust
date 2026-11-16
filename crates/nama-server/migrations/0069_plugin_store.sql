-- The Store each Automation Plugin keeps through the Kernel's functions (ADR 0048). Only
-- `src/automation/store.rs` reads or writes it. It outlives disabling the Plugin; deleting a User
-- deletes the entries scoped to them.
CREATE TABLE plugin_store (
    plugin_id TEXT NOT NULL,
    -- NULL for an entry that is the Plugin's alone, not one User's.
    user_id INTEGER REFERENCES users (id) ON DELETE CASCADE,
    key TEXT NOT NULL,
    value BLOB NOT NULL
) STRICT;

-- A unique index treats NULLs as distinct, so the Plugin-wide scope is folded to 0, which no User
-- id is.
CREATE UNIQUE INDEX plugin_store_entry ON plugin_store (plugin_id, ifnull(user_id, 0), key);
