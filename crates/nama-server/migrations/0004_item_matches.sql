-- What Matching found for each item: a Match to a title in a metadata source, with that title's
-- metadata, or that it stays Unmatched. An item with no row here hasn't been Matched yet, such
-- as when no metadata Provider was enabled or the Provider failed; the next scan tries again.
-- A file that changes is parsed again, which replaces its items and so their rows here.
CREATE TABLE item_matches (
    item_id INTEGER PRIMARY KEY NOT NULL REFERENCES library_items (id) ON DELETE CASCADE,
    -- 'matched', or 'unmatched' when no title fit confidently (or an ID hint named none).
    state TEXT NOT NULL CHECK (state IN ('matched', 'unmatched')),
    -- The id of the Plugin whose metadata Provider made the Match; image paths are its own.
    plugin TEXT,
    -- The Movie's or Episode's external ids, which Watch State sync keys on (ADR 0008).
    tmdb_id INTEGER,
    imdb_id TEXT,
    tvdb_id INTEGER,
    -- For an Episode, its Show's external ids.
    show_tmdb_id INTEGER,
    show_imdb_id TEXT,
    show_tvdb_id INTEGER,
    -- The Movie's or Episode's title, and for an Episode its Show's.
    title TEXT,
    original_title TEXT,
    show_title TEXT,
    overview TEXT,
    -- `YYYY-MM-DD`: a Movie's release or an Episode's first airing.
    release_date TEXT,
    runtime_minutes INTEGER,
    -- Image paths for the Provider's `image`. For an Episode, its Season's poster (or else its
    -- Show's) and its Show's backdrop.
    poster TEXT,
    backdrop TEXT,
    -- When Matching decided, in seconds since the Unix epoch.
    decided_at INTEGER NOT NULL,
    CHECK ((state = 'matched') = (plugin IS NOT NULL AND title IS NOT NULL))
) STRICT;
