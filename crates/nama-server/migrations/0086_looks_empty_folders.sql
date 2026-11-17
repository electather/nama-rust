-- Whether the folder's last scan found no file where an earlier one found some, so it kept what's
-- stored under it, as when a share is unmounted (#911). Until a scan finds files there again, or
-- an Admin has the next scan remove them.
ALTER TABLE library_folders ADD COLUMN looks_empty INTEGER NOT NULL DEFAULT 0 CHECK (looks_empty IN (0, 1));
