-- Each User's Watch State: how far they have got through a Movie or Episode, and whether it's
-- been played. Kept against the title's external id, the key the Hub syncs by (ADR 0008), never
-- against an Item or file, so it survives rescans, renames and new Versions (ADR 0013).
CREATE TABLE watch_states (
    user_id INTEGER NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    -- Movie and Episode TMDB ids are separate id spaces, so the kind is part of the key.
    kind TEXT NOT NULL CHECK (kind IN ('movie', 'episode')),
    -- The title's external id, as `tmdb-348`, `imdb-tt0078748` or `tvdb-81189`: whichever
    -- identifies it when browsing, its TMDB id if it has one.
    title_id TEXT NOT NULL,
    -- In milliseconds, as the player last reported them. The duration is zero if no player has.
    position_ms INTEGER NOT NULL CHECK (position_ms BETWEEN 0 AND duration_ms),
    duration_ms INTEGER NOT NULL CHECK (duration_ms >= 0),
    -- Set once the position passes 90% of the duration or it's marked played, and kept when it's
    -- played again. Marking it unplayed deletes the row.
    played INTEGER NOT NULL CHECK (played IN (0, 1)),
    -- When it last changed, in seconds since the Unix epoch.
    updated_at INTEGER NOT NULL,
    -- When each Watch State last became played, in seconds since the Unix epoch, which picks a Show's
    -- next Episode (#915); `updated_at` won't do, since rewatching a played Episode moves it. NULL
    -- while it's unplayed.
    played_at INTEGER,
    -- Whether the User hid the title from Continue Watching (#916). Any write to the row's Watch State
    -- clears it, so hiding lasts until the User plays the title again; a Show is hidden while the row
    -- that dates its entry is.
    hidden INTEGER NOT NULL DEFAULT 0 CHECK (hidden IN (0, 1)),
    PRIMARY KEY (user_id, kind, title_id)

) STRICT;

-- The Automation reads every User's Watch State of one title under any of its ids (ADR 0048), which
-- the primary key, led by `user_id`, can't serve.
CREATE INDEX watch_states_by_title ON watch_states (kind, title_id);
