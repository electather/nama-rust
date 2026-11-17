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
