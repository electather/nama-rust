-- Users, Devices and their credentials: Kernel state (ADR 0014). Times are seconds since the Unix
-- epoch. AUTOINCREMENT so a deleted User's or Device's id is never handed to a new one.

-- A person's identity on this Server.
CREATE TABLE users (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    -- As the User chose it, trimmed.
    name TEXT NOT NULL,
    -- `name` lowercased, so names are unique ignoring case. SQLite's NOCASE folds only ASCII.
    name_key TEXT NOT NULL UNIQUE,
    role TEXT NOT NULL CHECK (role IN ('admin', 'member')),
    created_at INTEGER NOT NULL
) STRICT;

-- One opaque credential string per User per Plugin, which only that Plugin's Authenticator can
-- make and check. For `local-auth` it is an argon2id hash.
CREATE TABLE user_credentials (
    user_id INTEGER NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    plugin TEXT NOT NULL,
    credential TEXT NOT NULL,
    PRIMARY KEY (user_id, plugin)
) STRICT;

-- A client installation signed in as a User. Deleting it revokes its session.
CREATE TABLE devices (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    -- The SHA-256 hash of the Device's session token. The token itself is never stored.
    token_hash BLOB NOT NULL UNIQUE,
    created_at INTEGER NOT NULL,
    last_seen_at INTEGER NOT NULL
) STRICT;

CREATE INDEX devices_by_user ON devices (user_id);
