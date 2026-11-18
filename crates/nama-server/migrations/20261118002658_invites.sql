-- A single-use link with which one person creates their own User. Used, revoked and dead ones are
-- deleted; expired ones linger until the next Invite is made, and are never listed or accepted.
CREATE TABLE invites (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    -- The SHA-256 hash of the token in its link. The token itself is never stored.
    token_hash BLOB NOT NULL UNIQUE,
    -- Who it's for, such as "Mum", or empty. Never the new User's name.
    note TEXT NOT NULL,
    -- A bitset like `users.permissions`. Accepting it grants these less any the creator has lost.
    permissions INTEGER NOT NULL,
    -- An Invite dies with its creator.
    created_by INTEGER NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    created_at INTEGER NOT NULL,
    expires_at INTEGER NOT NULL
) STRICT;

CREATE INDEX invites_by_creator ON invites (created_by);
