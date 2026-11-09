-- The version of the name parser that read each file (ADR 0023). A scan parses a file again when
-- it's older than the current one. Files stored before this column existed were read by version 1,
-- the current one when it was added, so adding it re-parses nothing.
ALTER TABLE library_files ADD COLUMN parser_version INTEGER NOT NULL DEFAULT 1;
