-- Matches an admin set or cleared by hand, which automatic Matching follows instead of searching.
--
-- A pin is keyed by what files name, as `library_items` stores it: a Movie's title, year and ID
-- hint, or those of the Show an Episode is in. So it outlasts the items it was set through: a file
-- that changes, or is renamed to a name that still says the same title, year and ID hint, or a new
-- Episode of a pinned Show, Matches the pinned title.
CREATE TABLE match_pins (
    library_id INTEGER NOT NULL REFERENCES libraries (id) ON DELETE CASCADE,
    kind TEXT NOT NULL CHECK (kind IN ('movie', 'episode')),
    title TEXT NOT NULL,
    year INTEGER,
    tmdb_id INTEGER,
    -- The id of the Movie, or for Episodes the Show, to Match. At most one is set; none means the
    -- items stay Unmatched.
    pinned_tmdb_id INTEGER,
    pinned_imdb_id TEXT,
    pinned_tvdb_id INTEGER,
    -- When the admin pinned it, in seconds since the Unix epoch.
    pinned_at INTEGER NOT NULL,
    CHECK ((pinned_tmdb_id IS NOT NULL) + (pinned_imdb_id IS NOT NULL)
           + (pinned_tvdb_id IS NOT NULL) <= 1)
) STRICT;

-- One pin per name. `year` and `tmdb_id` may be NULL, which a plain UNIQUE would let repeat.
CREATE UNIQUE INDEX match_pins_by_name
ON match_pins (library_id, kind, title, ifnull(year, -1), ifnull(tmdb_id, -1));
