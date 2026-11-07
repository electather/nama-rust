-- A User can mark a Movie or Episode played without playing it, so a Watch State may have no
-- duration yet: zero until a player reports one. SQLite can't change a CHECK in place, so the
-- table is rebuilt with `duration_ms >= 0`. Nothing references it, so dropping it is safe.
CREATE TABLE watch_states_new (
    user_id INTEGER NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    kind TEXT NOT NULL CHECK (kind IN ('movie', 'episode')),
    title_id TEXT NOT NULL,
    position_ms INTEGER NOT NULL CHECK (position_ms BETWEEN 0 AND duration_ms),
    -- Zero if no player has reported it.
    duration_ms INTEGER NOT NULL CHECK (duration_ms >= 0),
    -- Set once the position passes 90% of the duration or it's marked played, and kept when it's
    -- played again. Marking it unplayed deletes the row.
    played INTEGER NOT NULL CHECK (played IN (0, 1)),
    updated_at INTEGER NOT NULL,
    PRIMARY KEY (user_id, kind, title_id)
) STRICT;

INSERT INTO watch_states_new SELECT * FROM watch_states;
DROP TABLE watch_states;
ALTER TABLE watch_states_new RENAME TO watch_states;
