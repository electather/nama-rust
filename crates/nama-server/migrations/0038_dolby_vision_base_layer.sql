-- Dolby Vision's Base Layer: 'hdr10', 'hlg' or 'sdr', or NULL when it has none, as in profile 5.
-- It replaces whether the Base Layer is HDR10, which can't tell HLG from SDR.
ALTER TABLE tracks ADD COLUMN dolby_vision_base_layer TEXT
    CHECK (dolby_vision_base_layer IS NULL
           OR (hdr IS 'dolby_vision' AND dolby_vision_base_layer IN ('hdr10', 'hlg', 'sdr')));
-- Until probe version 4 reads them again, a Track whose Base Layer wasn't HDR10 has none.
UPDATE tracks SET dolby_vision_base_layer = 'hdr10' WHERE dolby_vision_hdr10_base_layer;
ALTER TABLE tracks DROP COLUMN dolby_vision_hdr10_base_layer;
