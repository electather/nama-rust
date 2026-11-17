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
    -- For an ambiguous item, the titles that tied, best fit first, as a JSON array of
    -- `[id, title, original_title, year]`, with the id written as an ID Hint is, such as `tmdb-2316`.
    tied_titles TEXT,
    -- The version of what the Server stores from the Provider that filled a Match, `METADATA_VERSION`
    -- in `src/matching.rs` (ADR 0040). A Scan fetches a Match stored by an older version again.
    metadata_version INTEGER NOT NULL DEFAULT 0,
    -- For an Episode, its Still: an image path from the Plugin's Provider, as `poster` is.
    still TEXT,
    genres TEXT,
    content_rating TEXT,
    -- For an Episode, its Show's own Poster (an image path, as `poster` is), overview and first air
    -- date. `poster` holds its Season's.
    show_poster TEXT,
    show_overview TEXT,
    show_first_air_date TEXT,
    -- The Plugin whose title source `image` serves `poster`, and the one serving `backdrop`, so a
    -- Match may keep images from more than one source. Set whenever its image is. An Episode's Still,
    -- and its Season's poster as browsing reads it, stay the Match's `plugin`'s.
    poster_plugin TEXT,
    backdrop_plugin TEXT,
    -- A Match's Title Logo and the Plugin whose title source `image` serves it, set together. For an
    -- Episode they hold its Show's logo, as `backdrop` holds its Show's backdrop.
    logo TEXT,
    logo_plugin TEXT,
    -- The Plugin whose title source serves `show_poster`, as `poster_plugin` is for `poster`.
    show_poster_plugin TEXT,
    -- Why automatic Matching left an item Unmatched, which says how an Admin fixes it: NULL for a
    -- Match, a pin, or another reason, such as an Episode its Show doesn't have.
    unmatched_reason TEXT
    CHECK (unmatched_reason IN ('ambiguous', 'no_title_fits', 'unknown_id_hint', 'not_in_show')),
    -- The title a Library's list sorts a Match under: its Show's for an Episode, its own for a Movie,
    -- lowercased and without accents or a leading English article (ADR 0055). The Server fills a
    -- NULL when it starts, since folding Unicode needs Rust, which SQL can't run.
    sort_title TEXT,
    CHECK ((state = 'matched') = (plugin IS NOT NULL AND title IS NOT NULL))

) STRICT;

-- `INSERT OR REPLACE` fires no delete trigger while `recursive_triggers` is off, so this drops
-- the row a re-Match replaces. Nothing updates `item_matches`; one that does needs a trigger too.
CREATE TRIGGER item_matches_searched AFTER INSERT ON item_matches
BEGIN
    DELETE FROM title_search WHERE rowid = NEW.item_id;
    INSERT INTO title_search (rowid, title, year, show_title)
    SELECT * FROM title_search_rows WHERE item_id = NEW.item_id;
END;

-- Also fires for the rows that removing a file, an item or a Library deletes by cascade.
CREATE TRIGGER item_matches_unsearched AFTER DELETE ON item_matches
BEGIN
    DELETE FROM title_search WHERE rowid = OLD.item_id;
END;
