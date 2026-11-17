-- Every id the metadata source gave for a Request's title when it was opened, the one it is kept
-- under among them: a Fulfiller finds the title by the namespace it knows (Sonarr by `tvdb`).
CREATE TABLE request_title_ids (
    request_id INTEGER NOT NULL REFERENCES requests (id) ON DELETE CASCADE,
    namespace TEXT NOT NULL CHECK (namespace != '' AND namespace NOT GLOB '*[^a-z0-9]*'),
    value TEXT NOT NULL CHECK (value != ''),
    PRIMARY KEY (request_id, namespace)
) STRICT, WITHOUT ROWID;
