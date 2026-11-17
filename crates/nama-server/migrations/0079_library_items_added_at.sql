-- When a scan first stored each Item, in seconds since the Unix epoch; a Reparse, or a scan of
-- its file changed in place, keeps it. Items stored before this migration get the time it ran, not
-- their file's mtime: Cleanup counts days unwatched from it (#837), so an mtime would make every
-- old title due at once. SQLite refuses a non-constant default here, so the scan sets it and 0 is
-- never kept.
ALTER TABLE library_items ADD COLUMN added_at INTEGER NOT NULL DEFAULT 0;
UPDATE library_items SET added_at = unixepoch();
