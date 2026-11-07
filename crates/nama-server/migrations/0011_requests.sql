-- Requests: Users asking for a Movie that isn't on the Server (ADR 0012). Kernel state. Times are
-- seconds since the Unix epoch.
CREATE TABLE requests (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    -- The Movie's TMDB id.
    tmdb_id INTEGER NOT NULL,
    -- The Movie's title and `YYYY-MM-DD` release date, as the metadata source gave them when the
    -- Request was opened: the Movie isn't in a Library to read them from.
    title TEXT NOT NULL,
    release_date TEXT,
    -- Every state of ADR 0012, so a later state needs no table rebuild to loosen this CHECK.
    state TEXT NOT NULL CHECK (state IN ('pending', 'approved', 'declined', 'processing',
                                         'partially_available', 'available', 'failed')),
    created_at INTEGER NOT NULL
) STRICT;

-- A Movie has at most one open Request. Declined and Available ones are closed history.
CREATE UNIQUE INDEX one_open_request_per_movie ON requests (tmdb_id)
WHERE state NOT IN ('declined', 'available');

-- Who asked for each Request.
CREATE TABLE requesters (
    request_id INTEGER NOT NULL REFERENCES requests (id) ON DELETE CASCADE,
    user_id INTEGER NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    requested_at INTEGER NOT NULL,
    PRIMARY KEY (request_id, user_id)
) STRICT;

CREATE INDEX requesters_by_user ON requesters (user_id);
