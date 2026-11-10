-- The most Transcodes the Server runs at once; Remuxes don't count.
ALTER TABLE server_settings ADD COLUMN max_transcodes INTEGER NOT NULL DEFAULT 2;
