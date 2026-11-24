-- A Device is signed in as several Users and acts as one of them at a time (ADR 0106). Each User
-- on it keeps how they signed in to it and when they last proved who they are there; the Device
-- keeps its token, expiry and the User it acts as now.
CREATE TABLE device_users (
    -- Names `devices` before the rebuild below, which keeps the name, so this key holds after it.
    device_id INTEGER NOT NULL REFERENCES devices (id) ON DELETE CASCADE,
    user_id INTEGER NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    -- How they signed in to it, or NULL for a Sign-in Code or an Account. Removing the Sign-in
    -- Method signs them out of it, but the cascade doesn't end their streams: remove one through
    -- `Users::write_signing_out`, returning these Devices' ids.
    sign_in_method_id INTEGER REFERENCES sign_in_methods (id) ON DELETE CASCADE,
    -- When they last proved who they are on it, or 0 if they haven't since signing in by a
    -- Sign-in Code or an Account, or since it last switched to them.
    authenticated_at INTEGER NOT NULL,
    PRIMARY KEY (device_id, user_id)
) STRICT;

CREATE INDEX device_users_by_user ON device_users (user_id);

CREATE INDEX device_users_by_sign_in_method ON device_users (sign_in_method_id);

INSERT INTO device_users (device_id, user_id, sign_in_method_id, authenticated_at)
SELECT id, user_id, sign_in_method_id, authenticated_at FROM devices;

-- SQLite can't make `user_id` nullable in place, so the table is rebuilt.
CREATE TABLE devices_new (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    -- The User it acts as now, one of its `device_users`, or NULL until one is picked.
    user_id INTEGER REFERENCES users (id) ON DELETE SET NULL,
    name TEXT NOT NULL,
    -- The SHA-256 hash of the Device's session token. The token itself is never stored.
    token_hash BLOB NOT NULL UNIQUE,
    -- The client IP address and User-Agent it first signed in from. Not updated on use.
    ip_address TEXT NOT NULL,
    user_agent TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    last_seen_at INTEGER NOT NULL,
    -- When its session stops working, for every User on it, unless used before then.
    expires_at INTEGER NOT NULL
) STRICT;

INSERT INTO devices_new (id, user_id, name, token_hash, ip_address, user_agent, created_at,
                         updated_at, last_seen_at, expires_at)
SELECT id, user_id, name, token_hash, ip_address, user_agent, created_at, updated_at,
       last_seen_at, expires_at
FROM devices;

DROP TABLE devices;

ALTER TABLE devices_new RENAME TO devices;

CREATE INDEX devices_by_user ON devices (user_id);

-- The last User leaving a Device signs it out, and the one it acts as leaving makes it act as
-- none, however they leave: a sign-out, a revoke, or a cascade from their User or Sign-in Method.
CREATE TRIGGER device_users_leave AFTER DELETE ON device_users
BEGIN
    UPDATE devices SET user_id = NULL WHERE id = OLD.device_id AND user_id = OLD.user_id;
    DELETE FROM devices
    WHERE id = OLD.device_id
        AND NOT EXISTS (SELECT 1 FROM device_users WHERE device_id = OLD.device_id);
END;
