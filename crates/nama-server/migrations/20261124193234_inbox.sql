-- Each User's inbox: every Request notification the Kernel told them (ADR 0021), kept whether
-- or not a Notifier delivered it. `kind` is a `nama.v1.NotificationKind` number, which never
-- changes under stored rows. Deleting the User deletes theirs.
CREATE TABLE inbox (
    id INTEGER PRIMARY KEY,
    user_id INTEGER NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    kind INTEGER NOT NULL CHECK (kind > 0),
    -- What the Request asked for, as it was then: a Movie's title, or a Show's and the Season's
    -- number.
    title TEXT NOT NULL,
    season_number INTEGER,
    -- No foreign key: the notification outlives a cancelled or withdrawn Request.
    request_id INTEGER NOT NULL,
    created_at INTEGER NOT NULL,
    -- NULL until the User reads it.
    read_at INTEGER
) STRICT;

CREATE INDEX inbox_by_user ON inbox (user_id, id);
