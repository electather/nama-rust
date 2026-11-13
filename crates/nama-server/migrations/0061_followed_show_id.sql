-- A Followed Show keys on its Show's External ID in its text form, so any namespace fits (ADR
-- 0042). Stored Followed Shows and their Requesters are dropped, not copied, as ADR 0042 says.
-- `followed_show_requesters` goes first: dropping `followed_shows` with foreign keys on would
-- otherwise delete its rows through the cascade. Dropping `followed_shows` sets
-- `requests.followed_show_id` to NULL through its ON DELETE SET NULL, which is right: the Season
-- Requests stay, and the Followed Show that opened them is gone.
DROP TABLE followed_show_requesters;
DROP TABLE followed_shows;

-- Followed Shows: Users asking for a whole Show, its Seasons now and later, approved once. Kernel
-- state beside `requests`: a Request is still one Season. Times are seconds since the Unix epoch.
CREATE TABLE followed_shows (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    -- The Show's External ID as `tmdb-1399`, and its title as the metadata source gave it when the
    -- Followed Show was opened, which the Season Requests it opens later take.
    show_id TEXT NOT NULL CHECK (show_id != ''),
    title TEXT NOT NULL,
    -- Cancelling or the last Requester withdrawing removes the row, so only declining closes one.
    state TEXT NOT NULL CHECK (state IN ('pending', 'approved', 'declined')),
    -- The highest Season number it covers, NULL if none: the Show's latest when the Followed Show
    -- was opened, then each new one a scan opened a Request for. Only a Season numbered after it
    -- is new, so a Season Request it opened and someone ended isn't opened again.
    latest_season INTEGER CHECK (latest_season >= 0),
    created_at INTEGER NOT NULL,
    decline_reason TEXT,
    -- A copy of the Quality a Followed Show's Season Requests take, as `requests` keeps one (ADR
    -- 0025), NULL if the Shows Fulfiller had none when it opened.
    quality_id INTEGER,
    quality_name TEXT,
    quality_profile_id TEXT
) STRICT;

-- A Show has at most one open Followed Show.
CREATE UNIQUE INDEX one_open_followed_show ON followed_shows (show_id) WHERE state != 'declined';

-- Who asked for each Followed Show. The Season Requests it opens later get these as Requesters.
CREATE TABLE followed_show_requesters (
    followed_show_id INTEGER NOT NULL REFERENCES followed_shows (id) ON DELETE CASCADE,
    user_id INTEGER NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    requested_at INTEGER NOT NULL,
    PRIMARY KEY (followed_show_id, user_id)
) STRICT;

CREATE INDEX followed_show_requesters_by_user ON followed_show_requesters (user_id);
