-- The Device each push subscription was made on, so signing its User out of that Device, however
-- it happens (a sign-out, a revoke from another Device, expiry, a cascade), deletes it with their
-- `device_users` row and pushes stop going there (#2314). A row from before has no Device, which
-- SQLite's foreign key leaves alone, and stays until its User unsubscribes or is deleted.
-- SQLite can't add a table's foreign key in place, so the table is rebuilt.
CREATE TABLE push_subscriptions_new (
    id INTEGER PRIMARY KEY,
    user_id INTEGER NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    device_id INTEGER,
    -- One browser's subscription: subscribed again by another User, it becomes theirs.
    endpoint TEXT NOT NULL UNIQUE,
    -- The browser's P-256 public key and authentication secret (RFC 8291).
    p256dh BLOB NOT NULL,
    auth BLOB NOT NULL,
    created_at INTEGER NOT NULL,
    FOREIGN KEY (device_id, user_id) REFERENCES device_users (device_id, user_id)
        ON DELETE CASCADE
) STRICT;

INSERT INTO push_subscriptions_new (id, user_id, endpoint, p256dh, auth, created_at)
SELECT id, user_id, endpoint, p256dh, auth, created_at FROM push_subscriptions;

DROP TABLE push_subscriptions;

ALTER TABLE push_subscriptions_new RENAME TO push_subscriptions;

CREATE INDEX push_subscriptions_by_user ON push_subscriptions (user_id);

-- The child key's index, so deleting a `device_users` row doesn't scan every subscription.
CREATE INDEX push_subscriptions_by_device_user ON push_subscriptions (device_id, user_id);
