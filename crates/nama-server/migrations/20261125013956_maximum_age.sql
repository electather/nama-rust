-- Each User's Maximum Age: the highest age a title's Content Rating may call for and still reach
-- them, NULL for none. `allow_unrated` lets a title with no Content Rating, or one the Server
-- doesn't know, reach a User with one.
ALTER TABLE users ADD COLUMN max_age INTEGER CHECK (max_age >= 0);
ALTER TABLE users ADD COLUMN allow_unrated INTEGER NOT NULL DEFAULT 0 CHECK (allow_unrated IN (0, 1));
