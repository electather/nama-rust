-- The `.nfo` files beside a Library's videos, each with the ID Hints it names (ADR 0087), so a
-- scan reads again only one that is new or whose size or modified time changed.
CREATE TABLE nfo_files (
    library_id INTEGER NOT NULL REFERENCES libraries (id) ON DELETE CASCADE,
    -- Absolute, under one of the Library's folders.
    path TEXT NOT NULL,
    size INTEGER NOT NULL,
    modified_ns INTEGER NOT NULL,
    -- A JSON array of ids as they display, strongest first, such as '["tmdb-603"]'; empty when
    -- the file names none or couldn't be read.
    ids TEXT NOT NULL,
    PRIMARY KEY (library_id, path)
) STRICT;

-- The ID Hints of an Item's `.nfo` file, as `nfo_files.ids` holds them, NULL without one. For a
-- Movie its own `.nfo`, for an Episode its Show's `tvshow.nfo`. They replace the name's.
ALTER TABLE library_items ADD COLUMN nfo_hints TEXT;
-- An Episode's own `.nfo` file's ID Hints, which name it among its Show's Episodes.
ALTER TABLE library_items ADD COLUMN episode_nfo_hints TEXT;
