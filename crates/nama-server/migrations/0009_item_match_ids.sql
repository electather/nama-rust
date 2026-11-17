-- Each id a Match gives its title, one row per namespace, so a title is found by any of its ids
-- and a namespace the Server doesn't know of yet needs no new column (ADR 0042). An Unmatched
-- Item has none.
CREATE TABLE item_match_ids (
    item_id INTEGER NOT NULL REFERENCES item_matches (item_id) ON DELETE CASCADE,
    -- 'title': the Movie's or Episode's own ids. 'show': for an Episode, its Show's.
    scope TEXT NOT NULL CHECK (scope IN ('title', 'show')),
    namespace TEXT NOT NULL CHECK (namespace != '' AND namespace NOT GLOB '*[^a-z0-9]*'),
    value TEXT NOT NULL CHECK (value != ''),
    PRIMARY KEY (item_id, scope, namespace)
) STRICT, WITHOUT ROWID;

-- Finds the Items Matched to a title by one of its ids.
CREATE INDEX item_match_ids_by_id ON item_match_ids (scope, namespace, value);
