-- Requests: Users asking for a Movie that isn't on the Server (ADR 0012). Kernel state. Times are
-- seconds since the Unix epoch.
CREATE TABLE requests (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    -- The Movie's, or for a Season its Show's, External ID as `tmdb-603`: the id the Request is
    -- kept under, one of its `request_title_ids`.
    title_id TEXT NOT NULL CHECK (title_id != ''),
    -- For a Season, its number; NULL for a Movie.
    season INTEGER CHECK (season >= 0),
    -- The Movie's, or the Show's, title, and the Movie's `YYYY-MM-DD` release date or the Season's
    -- first air date, as the metadata source gave them when the Request was opened: the title
    -- isn't in a Library to read them from.
    title TEXT NOT NULL,
    release_date TEXT,
    -- Every state of ADR 0012, so a later state needs no table rebuild to loosen this CHECK.
    state TEXT NOT NULL CHECK (state IN ('pending', 'approved', 'declined', 'processing',
                                         'partially_available', 'available', 'failed')),
    created_at INTEGER NOT NULL,
    -- Why a Request was declined, for its Requesters to read: NULL unless the decliner gave a
    -- reason.
    decline_reason TEXT,
    -- What the Fulfiller called a Request once it took it, which only that Fulfiller reads: NULL
    -- until a Fulfiller has taken the Request.
    fulfiller_reference TEXT,
    -- Why the Fulfiller didn't take a Request, in its own words: NULL unless the Request is Failed.
    failure_reason TEXT,
    -- The Followed Show that opened a Season Request. Ending the Followed Show leaves the Request.
    followed_show_id INTEGER REFERENCES followed_shows (id) ON DELETE SET NULL,
    -- A copy of the Quality a Request took when it opened, NULL if its Fulfiller had none. Kept as
    -- it was, so the Quality's renaming or removal changes neither what it reads nor its hand-off.
    quality_id INTEGER,
    quality_name TEXT,
    quality_profile_id TEXT,
    -- The Quality a Request's service downloads it at, as its hand-off reported it (ADR 0025). NULL
    -- until handed off, and when it took no Quality. `quality_used_id` is 0 when no Quality on the
    -- list had the profile, and the name is then the service's own: set in the service.
    quality_used_id INTEGER,
    quality_used_name TEXT,
    quality_used_profile_id TEXT,
    -- The latest change a Request manager made to a Request's Quality, all NULL if none: only the
    -- latest is kept. Who made it is forgotten when that User is deleted, as their Requests are.
    quality_changed_by INTEGER REFERENCES users (id) ON DELETE SET NULL,
    -- The friendly name of the Quality it had before, NULL if it had none.
    quality_changed_from TEXT,
    quality_changed_at INTEGER,
    -- The Plugin whose Fulfiller a Request was handed to at its latest Hand-off, so a Failed one
    -- can point at the Plugin to fix: NULL until it is handed off.
    fulfiller_plugin_id TEXT,
    -- The id namespaces its Fulfiller needed and the title had none of, comma-separated (`tvdb`), so
    -- the client API can say why the Request failed (ADR 0042). Only on a Request the Kernel failed
    -- for that without handing it off; NULL on every other.
    failure_needs TEXT
) STRICT;

CREATE UNIQUE INDEX one_open_request_per_movie_or_season ON requests (title_id, IFNULL(season, -1))
WHERE state NOT IN ('declined', 'available');

CREATE INDEX requests_by_followed_show ON requests (followed_show_id);
