-- The id of the Plugin keeping the image store's originals, or empty for local disk in the data
-- directory, where every Server before this keeps them (ADR 0052).
ALTER TABLE server_settings ADD COLUMN image_store_plugin TEXT NOT NULL DEFAULT '';
