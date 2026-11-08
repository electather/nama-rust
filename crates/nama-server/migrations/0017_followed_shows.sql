-- Followed Shows: Users asking for a whole Show, its Seasons now and later, approved once. Kernel
-- state beside `requests`: a Request is still one Season. Times are seconds since the Unix epoch.
CREATE TABLE followed_shows (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    -- The Show's TMDB id, and its title as the metadata source gave it when the Followed Show was
    -- opened, which the Season Requests it opens later take.
    tmdb_id INTEGER NOT NULL,
    title TEXT NOT NULL,
    -- Cancelling or the last Requester withdrawing removes the row, so only declining closes one.
    state TEXT NOT NULL CHECK (state IN ('pending', 'approved', 'declined')),
    -- The Show's highest Season number when the Followed Show was opened, NULL if it had none. Only
    -- a Season numbered after it is new, and gets a Request when a scan Matches its Episodes.
    latest_season INTEGER CHECK (latest_season >= 0),
    created_at INTEGER NOT NULL,
    decline_reason TEXT
) STRICT;

-- A Show has at most one open Followed Show.
CREATE UNIQUE INDEX one_open_followed_show ON followed_shows (tmdb_id) WHERE state != 'declined';

-- Who asked for each Followed Show. The Season Requests it opens later get these as Requesters.
CREATE TABLE followed_show_requesters (
    followed_show_id INTEGER NOT NULL REFERENCES followed_shows (id) ON DELETE CASCADE,
    user_id INTEGER NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    requested_at INTEGER NOT NULL,
    PRIMARY KEY (followed_show_id, user_id)
) STRICT;

CREATE INDEX followed_show_requesters_by_user ON followed_show_requesters (user_id);

-- The Followed Show that opened a Season Request. Ending the Followed Show leaves the Request.
ALTER TABLE requests ADD COLUMN followed_show_id INTEGER
    REFERENCES followed_shows (id) ON DELETE SET NULL;
