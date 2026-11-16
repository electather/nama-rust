-- A Request keys on one External ID in its text form and keeps the title's other ids in a child
-- table, so any namespace fits (ADR 0042). Stored Requests and their Requesters are dropped, not
-- copied, as ADR 0042 says. `requesters` goes first: dropping `requests` with foreign keys on would
-- otherwise delete its rows through the cascade. Nothing else references `requests`.
DROP TABLE requesters;
DROP TABLE requests;

-- Requests: Users asking for a Movie, or one Season of a Show, that isn't on the Server (ADR 0012).
-- Kernel state. Times are seconds since the Unix epoch.
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
    fulfiller_plugin_id TEXT
) STRICT;

-- A Movie, or a Season, has at most one open Request. Declined and Available ones are closed
-- history. Movies and Shows have separate id spaces, so a Movie and a Show may share a `title_id`;
-- -1 stands for a Movie's missing Season, since a unique index counts every NULL as distinct.
CREATE UNIQUE INDEX one_open_request_per_movie_or_season ON requests (title_id, IFNULL(season, -1))
WHERE state NOT IN ('declined', 'available');

CREATE INDEX requests_by_followed_show ON requests (followed_show_id);

-- Every id the metadata source gave for a Request's title when it was opened, the one it is kept
-- under among them: a Fulfiller finds the title by the namespace it knows (Sonarr by `tvdb`).
CREATE TABLE request_title_ids (
    request_id INTEGER NOT NULL REFERENCES requests (id) ON DELETE CASCADE,
    namespace TEXT NOT NULL,
    value TEXT NOT NULL CHECK (value != ''),
    PRIMARY KEY (request_id, namespace)
) STRICT, WITHOUT ROWID;

-- Who asked for each Request.
CREATE TABLE requesters (
    request_id INTEGER NOT NULL REFERENCES requests (id) ON DELETE CASCADE,
    user_id INTEGER NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    requested_at INTEGER NOT NULL,
    PRIMARY KEY (request_id, user_id)
) STRICT;

CREATE INDEX requesters_by_user ON requesters (user_id);
