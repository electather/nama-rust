-- Libraries: named collections of one kind of media, each backed by one or more folders.
-- AUTOINCREMENT so a deleted Library's id is never handed to a new one: a client still holding
-- the old id gets `not_found` instead of a different Library.
CREATE TABLE libraries (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    kind TEXT NOT NULL CHECK (kind IN ('movies', 'shows'))
) STRICT;

-- A Library holds one kind of media for its whole life.
CREATE TRIGGER libraries_kind_is_fixed
BEFORE UPDATE OF kind ON libraries
WHEN NEW.kind IS NOT OLD.kind
BEGIN
    SELECT RAISE(ABORT, 'a Library''s kind can''t change');
END;

-- The folders backing each Library, as canonical absolute paths, in the order the admin gave
-- them. The primary key keeps a folder from belonging to two Libraries.
CREATE TABLE library_folders (
    path TEXT PRIMARY KEY NOT NULL,
    library_id INTEGER NOT NULL REFERENCES libraries (id) ON DELETE CASCADE,
    position INTEGER NOT NULL
) STRICT;

CREATE INDEX library_folders_by_library ON library_folders (library_id, position);
