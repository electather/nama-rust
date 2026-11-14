-- A namespace is lowercase ASCII letters and digits (ADR 0042), so SQLite checks its form too, not
-- only the Rust type. SQLite can't add a CHECK in place, so both id tables are rebuilt. Nothing
-- references either, so dropping one deletes no other row through a cascade.
CREATE TABLE item_match_ids_new (
    item_id INTEGER NOT NULL REFERENCES item_matches (item_id) ON DELETE CASCADE,
    -- 'title': the Movie's or Episode's own ids. 'show': for an Episode, its Show's.
    scope TEXT NOT NULL CHECK (scope IN ('title', 'show')),
    namespace TEXT NOT NULL CHECK (namespace != '' AND namespace NOT GLOB '*[^a-z0-9]*'),
    value TEXT NOT NULL CHECK (value != ''),
    PRIMARY KEY (item_id, scope, namespace)
) STRICT, WITHOUT ROWID;

INSERT INTO item_match_ids_new SELECT * FROM item_match_ids;
DROP TABLE item_match_ids;
ALTER TABLE item_match_ids_new RENAME TO item_match_ids;
-- Finds the Items Matched to a title by one of its ids.
CREATE INDEX item_match_ids_by_id ON item_match_ids (scope, namespace, value);

-- Every id the metadata source gave for a Request's title when it was opened, the one it is kept
-- under among them: a Fulfiller finds the title by the namespace it knows (Sonarr by `tvdb`).
CREATE TABLE request_title_ids_new (
    request_id INTEGER NOT NULL REFERENCES requests (id) ON DELETE CASCADE,
    namespace TEXT NOT NULL CHECK (namespace != '' AND namespace NOT GLOB '*[^a-z0-9]*'),
    value TEXT NOT NULL CHECK (value != ''),
    PRIMARY KEY (request_id, namespace)
) STRICT, WITHOUT ROWID;

INSERT INTO request_title_ids_new SELECT * FROM request_title_ids;
DROP TABLE request_title_ids;
ALTER TABLE request_title_ids_new RENAME TO request_title_ids;
