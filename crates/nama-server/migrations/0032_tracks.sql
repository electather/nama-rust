-- One row per video, audio or subtitle stream, in the order the file holds them.
CREATE TABLE tracks (
    version_id INTEGER NOT NULL REFERENCES version_probes (version_id) ON DELETE CASCADE,
    position INTEGER NOT NULL,
    kind TEXT NOT NULL CHECK (kind IN ('video', 'audio', 'subtitle')),
    codec TEXT NOT NULL,
    -- As the file tags it, usually ISO 639-2, such as `eng`.
    language TEXT,
    title TEXT,
    is_default INTEGER NOT NULL,
    forced INTEGER NOT NULL,
    width INTEGER CHECK ((kind = 'video') OR width IS NULL),
    height INTEGER CHECK ((kind = 'video') OR height IS NULL),
    frame_rate REAL CHECK ((kind = 'video') OR frame_rate IS NULL),
    channels INTEGER CHECK ((kind = 'audio') OR channels IS NULL),
    -- A bitmap subtitle, such as PGS, rather than text, such as SRT.
    image INTEGER CHECK ((kind = 'subtitle') = (image IS NOT NULL)),
    -- A video Track's bit depth and HDR format. Tracks read by probe version 1 have neither.
    bit_depth INTEGER CHECK ((kind = 'video') OR bit_depth IS NULL),
    -- NULL for SDR, or when the file doesn't say.
    hdr TEXT
    CHECK (hdr IS NULL OR (kind = 'video' AND hdr IN ('hdr10', 'hlg', 'dolby_vision'))),
    -- Dolby Vision's profile, such as 8, and whether its base layer is HDR10, which a player without
    -- Dolby Vision shows instead. Set for Dolby Vision only.
    dolby_vision_profile INTEGER
    CHECK ((hdr IS 'dolby_vision') = (dolby_vision_profile IS NOT NULL)),
    -- Whether the file marks a Track for the deaf and hard of hearing (SDH). Tracks read by probe
    -- version 2 or older have 0 until probed again.
    sdh INTEGER NOT NULL DEFAULT 0,
    -- Dolby Vision's Base Layer: 'hdr10', 'hlg' or 'sdr', or NULL when it has none, as in profile 5.
    -- Not a yes/no for HDR10, which couldn't tell HLG from SDR.
    dolby_vision_base_layer TEXT
    CHECK (dolby_vision_base_layer IS NULL
           OR (hdr IS 'dolby_vision' AND dolby_vision_base_layer IN ('hdr10', 'hlg', 'sdr'))),
    -- Whether the file carries a subtitle Track's codec header, such as VobSub's `.idx` with its
    -- palette and size. NULL for a Track that isn't a subtitle, and for one probed before it, until
    -- PROBE_VERSION 8 probes its file again.
    header INTEGER,
    PRIMARY KEY (version_id, position)

) STRICT;
