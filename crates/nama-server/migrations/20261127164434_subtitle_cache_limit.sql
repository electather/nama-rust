-- The most MiB the subtitle cache takes, or NULL for the default (#2495).
ALTER TABLE server_settings ADD COLUMN subtitle_cache_mib INTEGER;
