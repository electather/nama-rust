-- The video files in each Library's folders, as of its last scan, and the Movies and Episodes
-- their names say they hold.
--
-- A file is Unmatched until Matching links it to a title in a metadata source. A file whose name
-- doesn't follow the naming conventions has no items, so nothing can Match it automatically.

-- One row per video file per Library. `size` and `modified_ns` (nanoseconds since the Unix
-- epoch) are what a rescan compares to skip a file that hasn't changed.
CREATE TABLE library_files (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    library_id INTEGER NOT NULL REFERENCES libraries (id) ON DELETE CASCADE,
    -- Absolute path, under one of the Library's folders.
    path TEXT NOT NULL,
    size INTEGER NOT NULL,
    modified_ns INTEGER NOT NULL,
    UNIQUE (library_id, path)
) STRICT;

-- What a file holds: one Movie, or one or more Episodes of a Show. For an Episode, `title`,
-- `year` and `tmdb_id` are the Show's. `tmdb_id` comes from an ID hint in the name.
CREATE TABLE library_items (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    file_id INTEGER NOT NULL REFERENCES library_files (id) ON DELETE CASCADE,
    kind TEXT NOT NULL CHECK (kind IN ('movie', 'episode')),
    title TEXT NOT NULL,
    year INTEGER,
    tmdb_id INTEGER,
    season INTEGER CHECK ((kind = 'episode') = (season IS NOT NULL)),
    episode INTEGER CHECK ((kind = 'episode') = (episode IS NOT NULL))
) STRICT;

CREATE INDEX library_items_by_file ON library_items (file_id);
