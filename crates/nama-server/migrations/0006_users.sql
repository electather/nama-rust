-- Users, their Sign-in Methods and Devices: Kernel state (ADR 0014). Names follow better-auth's
-- where they don't clash with GLOSSARY.md. Times are seconds since the Unix epoch. AUTOINCREMENT so
-- a deleted row's id is never handed to a new one.

-- A person's identity on this Server.
CREATE TABLE users (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    -- `display_username` lowercased, so names are unique ignoring case. SQLite's NOCASE folds only
    -- ASCII.
    username TEXT NOT NULL UNIQUE,
    -- As the User chose it, trimmed. The API calls it the User's name.
    display_username TEXT NOT NULL,
    role TEXT NOT NULL CHECK (role IN ('admin', 'member')),
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
) STRICT;

-- A way a User proves who they are. Plugins have no storage, so the Kernel keeps the credential
-- for the Plugin `provider_id`, which alone can make and check it (for `local-auth`, an argon2id
-- hash).
CREATE TABLE sign_in_methods (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    provider_id TEXT NOT NULL,
    -- NULL for a password; otherwise who the person is to the provider, by which the User is found.
    -- UNIQUE treats NULLs as distinct, so this constrains only the non-passwords.
    subject TEXT,
    credential TEXT NOT NULL,
    -- What the User is shown, such as "Password".
    name TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    UNIQUE (provider_id, subject)
) STRICT;

CREATE INDEX sign_in_methods_by_user ON sign_in_methods (user_id);

-- At most one password per User. Changing it updates this row, so the Devices that signed in with
-- it stay signed in.
CREATE UNIQUE INDEX one_password_per_user ON sign_in_methods (user_id) WHERE subject IS NULL;

-- A User always keeps a way in. Deleting the User, which cascades here, is still allowed.
CREATE TRIGGER sign_in_methods_keep_one BEFORE DELETE ON sign_in_methods
WHEN NOT EXISTS (SELECT 1 FROM sign_in_methods WHERE user_id = OLD.user_id AND id != OLD.id)
    AND EXISTS (SELECT 1 FROM users WHERE id = OLD.user_id)
BEGIN
    SELECT RAISE(ABORT, 'a User must keep at least one Sign-in Method');
END;

-- A client installation signed in as a User. Deleting it revokes its session.
CREATE TABLE devices (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    -- How it signed in, or NULL for a Sign-in Code. Removing the Sign-in Method signs it out.
    sign_in_method_id INTEGER REFERENCES sign_in_methods (id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    -- The SHA-256 hash of the Device's session token. The token itself is never stored.
    token_hash BLOB NOT NULL UNIQUE,
    -- The client IP address and User-Agent it signed in from. Not updated on use.
    ip_address TEXT NOT NULL,
    user_agent TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    last_seen_at INTEGER NOT NULL,
    -- When its session stops working unless used before then.
    expires_at INTEGER NOT NULL,
    -- When it last proved who it is, or 0 if it never has, as a Device signed in by a Sign-in Code.
    authenticated_at INTEGER NOT NULL
) STRICT;

CREATE INDEX devices_by_user ON devices (user_id);
CREATE INDEX devices_by_sign_in_method ON devices (sign_in_method_id);
