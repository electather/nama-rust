-- The folders backing each Library, as canonical absolute paths, in the order the admin gave
-- them. The primary key keeps a folder from belonging to two Libraries.
CREATE TABLE library_folders (
    path TEXT PRIMARY KEY NOT NULL,
    library_id INTEGER NOT NULL REFERENCES libraries (id) ON DELETE CASCADE,
    position INTEGER NOT NULL,
    -- Whether the Server couldn't write to the folder when it last scanned its Library, so media in it
    -- isn't deleted (ADR 0038). Writable until a scan finds otherwise.
    read_only INTEGER NOT NULL DEFAULT 0 CHECK (read_only IN (0, 1)),
    -- Whether the folder's last scan found no file where an earlier one found some, so it kept what's
    -- stored under it, as when a share is unmounted (#911). Until a scan finds files there again, or
    -- an Admin has the next scan remove them.
    looks_empty INTEGER NOT NULL DEFAULT 0 CHECK (looks_empty IN (0, 1))
) STRICT;

CREATE INDEX library_folders_by_library ON library_folders (library_id, position);
