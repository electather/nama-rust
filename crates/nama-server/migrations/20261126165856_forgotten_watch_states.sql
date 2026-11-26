-- When each User last marked a Movie or Episode unplayed, under each of its ids, since marking
-- unplayed deletes its `watch_states` rows and a pulled or imported change made before it must not
-- bring it back (#2266).
CREATE TABLE forgotten_watch_states (
    user_id INTEGER NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    kind TEXT NOT NULL CHECK (kind IN ('movie', 'episode')),
    title_id TEXT NOT NULL,
    -- In seconds since the Unix epoch.
    forgotten_at INTEGER NOT NULL,
    PRIMARY KEY (user_id, kind, title_id)
) STRICT;
