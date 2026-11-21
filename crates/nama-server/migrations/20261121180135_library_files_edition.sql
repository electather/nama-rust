-- The Edition a Version holds, as its file or folder name writes it (`{edition-Director's Cut}`),
-- or NULL. Files stored before it get theirs when the next scan parses them again (ADR 0023).
ALTER TABLE library_files ADD COLUMN edition TEXT;
