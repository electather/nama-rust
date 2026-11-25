-- The Server's VAPID key for Web Push (RFC 8292): a P-256 key in PKCS#8, made on the Server's first
-- start and kept for good, since every browser subscribed with its public key. Never logged or
-- sent to a client.
ALTER TABLE server_settings ADD COLUMN web_push_key BLOB;

-- Each browser's push subscription (RFC 8030), which the Server pushes its User's inbox
-- notifications to. Deleting the User deletes theirs.
CREATE TABLE push_subscriptions (
    id INTEGER PRIMARY KEY,
    user_id INTEGER NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    -- One browser's subscription: subscribed again by another User, it becomes theirs.
    endpoint TEXT NOT NULL UNIQUE,
    -- The browser's P-256 public key and authentication secret (RFC 8291).
    p256dh BLOB NOT NULL,
    auth BLOB NOT NULL,
    created_at INTEGER NOT NULL
) STRICT;

CREATE INDEX push_subscriptions_by_user ON push_subscriptions (user_id);
