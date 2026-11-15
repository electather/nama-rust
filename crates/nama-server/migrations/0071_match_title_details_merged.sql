-- 0055 (from main) and 0067 (this branch) both added a Title Logo to `item_matches`. `logo`, with
-- the Plugin serving it in `logo_plugin`, is the one kept, since artwork may come from any of a
-- Library's title sources (ADR 0045); a Match with only 0055's `title_logo` keeps it there.
UPDATE item_matches SET logo = title_logo, logo_plugin = plugin
WHERE logo IS NULL AND title_logo IS NOT NULL;
ALTER TABLE item_matches DROP COLUMN title_logo;
-- The Plugin whose title source serves `show_poster` (0055), as `poster_plugin` is for `poster`.
ALTER TABLE item_matches ADD COLUMN show_poster_plugin TEXT;
UPDATE item_matches SET show_poster_plugin = plugin WHERE show_poster IS NOT NULL;
