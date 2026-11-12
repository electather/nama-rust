-- Whether Posters and Backdrops are resized on request, and the most MiB the resized copies take,
-- or NULL for the default.
ALTER TABLE server_settings ADD COLUMN resize_images INTEGER NOT NULL DEFAULT 1;
ALTER TABLE server_settings ADD COLUMN resized_cache_mib INTEGER;
