-- Each id a Match gives its title, one row per namespace, so a title is found by any of its ids
-- and a namespace the Server doesn't know of yet needs no new column (ADR 0042). Replaces the six
-- id columns of `item_matches`, which a later migration drops. An Unmatched Item has none.
CREATE TABLE item_match_ids (
    item_id INTEGER NOT NULL REFERENCES item_matches (item_id) ON DELETE CASCADE,
    -- 'title': the Movie's or Episode's own ids. 'show': for an Episode, its Show's.
    scope TEXT NOT NULL CHECK (scope IN ('title', 'show')),
    namespace TEXT NOT NULL,
    value TEXT NOT NULL CHECK (value != ''),
    PRIMARY KEY (item_id, scope, namespace)
) STRICT, WITHOUT ROWID;
-- Finds the Items Matched to a title by one of its ids.
CREATE INDEX item_match_ids_by_id ON item_match_ids (scope, namespace, value);

-- Existing Matches keep their ids.
INSERT INTO item_match_ids
SELECT item_id, 'title', 'tmdb', CAST(tmdb_id AS TEXT) FROM item_matches WHERE tmdb_id IS NOT NULL;
INSERT INTO item_match_ids
SELECT item_id, 'title', 'imdb', imdb_id FROM item_matches WHERE imdb_id IS NOT NULL;
INSERT INTO item_match_ids
SELECT item_id, 'title', 'tvdb', CAST(tvdb_id AS TEXT) FROM item_matches WHERE tvdb_id IS NOT NULL;
INSERT INTO item_match_ids
SELECT item_id, 'show', 'tmdb', CAST(show_tmdb_id AS TEXT) FROM item_matches
WHERE show_tmdb_id IS NOT NULL;
INSERT INTO item_match_ids
SELECT item_id, 'show', 'imdb', show_imdb_id FROM item_matches WHERE show_imdb_id IS NOT NULL;
INSERT INTO item_match_ids
SELECT item_id, 'show', 'tvdb', CAST(show_tvdb_id AS TEXT) FROM item_matches
WHERE show_tvdb_id IS NOT NULL;
