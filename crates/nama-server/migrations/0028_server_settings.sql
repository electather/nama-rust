-- What an admin sets about the Server as a whole, in at most one row: none until it is first set.
CREATE TABLE server_settings (
    id INTEGER PRIMARY KEY NOT NULL CHECK (id = 1),
    -- The address people reach the Server at, an http or https URL, or '' for none.
    url TEXT NOT NULL,
    -- The folder playback writes segment files to, an absolute path, or '' for data_dir/transcodes.
    segment_folder TEXT NOT NULL DEFAULT '',
    -- The Playback Limit for remote playback, by its name, or '' for Original.
    remote_playback_limit TEXT NOT NULL DEFAULT ''
    CHECK (remote_playback_limit IN ('', '1080p', '720p', '480p')),
    -- What Transcodes run on: '' for Automatic, 'off', or an API such as 'vaapi'.
    hardware_acceleration TEXT NOT NULL DEFAULT '',
    -- The most Transcodes the Server runs at once; Remuxes don't count.
    max_transcodes INTEGER,
    -- The GPU `hardware_acceleration` runs on, by its PCI address, or '' for none, and the name it was
    -- last found under, which still names it once it's gone.
    hardware_gpu TEXT NOT NULL DEFAULT '',
    hardware_gpu_name TEXT NOT NULL DEFAULT '',
    -- How many minutes behind playback a running Remux or Transcode keeps its files, or NULL for the
    -- default.
    keep_behind_minutes INTEGER,
    -- Whether Posters and Backdrops are resized on request, and the most MiB the resized copies take,
    -- or NULL for the default.
    resize_images INTEGER NOT NULL DEFAULT 1,
    resized_cache_mib INTEGER,
    -- The id of the Plugin keeping the image store's originals, or empty for local disk in the data
    -- directory (ADR 0052).
    image_store_plugin TEXT NOT NULL DEFAULT ''
) STRICT;
