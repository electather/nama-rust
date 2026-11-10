-- A video Track's bit depth and HDR format. Tracks read by probe version 1 have neither.
ALTER TABLE tracks ADD COLUMN bit_depth INTEGER CHECK ((kind = 'video') OR bit_depth IS NULL);
-- NULL for SDR, or when the file doesn't say.
ALTER TABLE tracks ADD COLUMN hdr TEXT
    CHECK (hdr IS NULL OR (kind = 'video' AND hdr IN ('hdr10', 'hlg', 'dolby_vision')));
-- Dolby Vision's profile, such as 8, and whether its base layer is HDR10, which a player without
-- Dolby Vision shows instead. Set for Dolby Vision only.
ALTER TABLE tracks ADD COLUMN dolby_vision_profile INTEGER
    CHECK ((hdr IS 'dolby_vision') = (dolby_vision_profile IS NOT NULL));
ALTER TABLE tracks ADD COLUMN dolby_vision_hdr10_base_layer INTEGER
    CHECK ((hdr IS 'dolby_vision') = (dolby_vision_hdr10_base_layer IS NOT NULL));
