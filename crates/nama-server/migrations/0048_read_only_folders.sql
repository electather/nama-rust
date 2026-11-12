-- Whether the Server couldn't write to the folder when it last scanned its Library, so media in it
-- isn't deleted (ADR 0038). Writable until a scan finds otherwise.
ALTER TABLE library_folders ADD COLUMN read_only INTEGER NOT NULL DEFAULT 0 CHECK (read_only IN (0, 1));
