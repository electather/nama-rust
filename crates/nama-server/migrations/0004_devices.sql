-- A client installation signed in as a User. Deleting it revokes its session.
CREATE TABLE devices (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    -- How it signed in, or NULL for a Sign-in Code. Removing the Sign-in Method signs it out, but
    -- the cascade doesn't end its streams: remove one through `Users::write_signing_out`, returning
    -- these Devices' ids.
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
