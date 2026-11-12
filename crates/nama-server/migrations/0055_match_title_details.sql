-- A Movie's, or for an Episode its Show's, Title Logo (an image path from the Plugin's Provider, as
-- `poster` is), genres (a JSON array of their names) and Content Rating.
ALTER TABLE item_matches ADD COLUMN title_logo TEXT;
ALTER TABLE item_matches ADD COLUMN genres TEXT;
ALTER TABLE item_matches ADD COLUMN content_rating TEXT;
-- For an Episode, its Show's own Poster (an image path, as `poster` is), overview and first air
-- date. `poster` holds its Season's.
ALTER TABLE item_matches ADD COLUMN show_poster TEXT;
ALTER TABLE item_matches ADD COLUMN show_overview TEXT;
ALTER TABLE item_matches ADD COLUMN show_first_air_date TEXT;
