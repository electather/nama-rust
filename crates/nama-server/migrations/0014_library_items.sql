-- What a file holds: one Movie, or one or more Episodes of a Show. For an Episode, `title`,
-- `year` and `hint` are the Show's.
CREATE TABLE library_items (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    file_id INTEGER NOT NULL REFERENCES library_files (id) ON DELETE CASCADE,
    kind TEXT NOT NULL CHECK (kind IN ('movie', 'episode')),
    title TEXT NOT NULL,
    year INTEGER,
    season INTEGER CHECK ((kind = 'episode') = (season IS NOT NULL)),
    episode INTEGER CHECK ((kind = 'episode') = (episode IS NOT NULL)),
    -- An ID Hint may name a title at TMDB, TVDB or IMDb, so items and pins store it as the id it
    -- names, such as `tmdb-78`, `tvdb-433631` or `imdb-tt0083658`.
    hint TEXT,
    -- When a scan first stored each Item, in seconds since the Unix epoch; a Reparse, or a scan of
    -- its file changed in place, keeps it. Not the file's mtime: Cleanup counts days unwatched from
    -- it (#837), so an mtime would make every old title due at once. SQLite refuses a non-constant
    -- default here, so the scan sets it and 0 is never kept.
    added_at INTEGER NOT NULL DEFAULT 0
) STRICT;

CREATE INDEX library_items_by_file ON library_items (file_id);
