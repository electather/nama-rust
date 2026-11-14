-- Every id the metadata source gave for a Followed Show's Show when it was opened, the one it is
-- kept under among them, so the Season Requests a scan opens for it carry them too: a Fulfiller
-- finds the Show by the namespace it knows (Sonarr by `tvdb`).
CREATE TABLE followed_show_ids (
    followed_show_id INTEGER NOT NULL REFERENCES followed_shows (id) ON DELETE CASCADE,
    namespace TEXT NOT NULL CHECK (namespace != '' AND namespace NOT GLOB '*[^a-z0-9]*'),
    value TEXT NOT NULL CHECK (value != ''),
    PRIMARY KEY (followed_show_id, namespace)
) STRICT, WITHOUT ROWID;

-- Finds the Followed Shows of a Show by one of its ids.
CREATE INDEX followed_show_ids_by_id ON followed_show_ids (namespace, value);

-- A Followed Show opened before this knows only the id it is kept under, `namespace-value` with no
-- `-` in the namespace (ADR 0042).
INSERT INTO followed_show_ids (followed_show_id, namespace, value)
SELECT id, substr(show_id, 1, instr(show_id, '-') - 1), substr(show_id, instr(show_id, '-') + 1)
FROM followed_shows;
