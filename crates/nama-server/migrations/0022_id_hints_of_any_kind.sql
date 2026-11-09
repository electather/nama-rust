-- An ID Hint may name a title at TMDB, TVDB or IMDb, so items and pins store it as the id it
-- names, such as `tmdb-78`, `tvdb-433631` or `imdb-tt0083658`, instead of a bare TMDB id.
ALTER TABLE library_items ADD COLUMN hint TEXT;
UPDATE library_items SET hint = 'tmdb-' || tmdb_id WHERE tmdb_id IS NOT NULL;
ALTER TABLE library_items DROP COLUMN tmdb_id;

-- The index names `tmdb_id`, which SQLite won't drop while it's indexed.
DROP INDEX match_pins_by_name;
ALTER TABLE match_pins ADD COLUMN hint TEXT;
UPDATE match_pins SET hint = 'tmdb-' || tmdb_id WHERE tmdb_id IS NOT NULL;
ALTER TABLE match_pins DROP COLUMN tmdb_id;
-- One pin per name. `year` and `hint` may be NULL, which a plain UNIQUE would let repeat.
CREATE UNIQUE INDEX match_pins_by_name
ON match_pins (library_id, kind, title, ifnull(year, -1), ifnull(hint, ''));
