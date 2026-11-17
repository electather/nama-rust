-- A person's identity on this Server.
CREATE TABLE users (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    -- `display_username` lowercased, so names are unique ignoring case. SQLite's NOCASE folds only
    -- ASCII.
    username TEXT NOT NULL UNIQUE,
    -- As the User chose it, trimmed. The API calls it the User's name.
    display_username TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    -- Permissions (ADR 0020). `permissions` is a bitset: bit n is the `nama.v1.Permission`
    -- numbered n, so ADMIN (1) is 2, and REQUEST (6) with VIEW_REQUESTS (12) is 64 + 4096. Every insert
    -- sets it, so the default is never kept.
    permissions INTEGER NOT NULL DEFAULT 0
) STRICT;
