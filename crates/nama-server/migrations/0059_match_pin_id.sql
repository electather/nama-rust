-- A pin keys on one id, so it holds it in one text column (ADR 0042). The old columns are named
-- in a table CHECK, which ALTER can't drop, so the table is rebuilt. Nothing references it.
CREATE TABLE match_pins_new (
    library_id INTEGER NOT NULL REFERENCES libraries (id) ON DELETE CASCADE,
    kind TEXT NOT NULL CHECK (kind IN ('movie', 'episode')),
    title TEXT NOT NULL,
    year INTEGER,
    hint TEXT,
    -- The pinned Movie's, or for Episodes the Show's, id as `tmdb-603`; NULL means the Items stay
    -- Unmatched.
    pinned_id TEXT,
    -- When the admin pinned it, in seconds since the Unix epoch.
    pinned_at INTEGER NOT NULL
) STRICT;

INSERT INTO match_pins_new (library_id, kind, title, year, hint, pinned_id, pinned_at)
SELECT library_id, kind, title, year, hint,
       COALESCE('tmdb-' || pinned_tmdb_id, 'imdb-' || pinned_imdb_id, 'tvdb-' || pinned_tvdb_id),
       pinned_at
FROM match_pins;

DROP TABLE match_pins;
ALTER TABLE match_pins_new RENAME TO match_pins;

-- One pin per name. `year` and `hint` may be NULL, which a plain UNIQUE would let repeat.
CREATE UNIQUE INDEX match_pins_by_name
ON match_pins (library_id, kind, title, ifnull(year, -1), ifnull(hint, ''));
