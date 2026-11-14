-- The Plugin whose title source `image` serves `poster`, and the one serving `backdrop`, so a
-- Match may keep images from more than one source. Set whenever its image is. An Episode's Still,
-- and its Season's poster as browsing reads it, stay the Match's `plugin`'s, as every image was.
ALTER TABLE item_matches ADD COLUMN poster_plugin TEXT;
ALTER TABLE item_matches ADD COLUMN backdrop_plugin TEXT;
UPDATE item_matches SET poster_plugin = plugin WHERE poster IS NOT NULL;
UPDATE item_matches SET backdrop_plugin = plugin WHERE backdrop IS NOT NULL;
