-- The Content Rating of the title a Request or Followed Show asks for, as the title source gave it
-- when the row opened, so a User's Maximum Age can keep others' Requests for titles over it from
-- them (#1414). NULL for none, and for every row opened before this, which counts as unrated.
ALTER TABLE requests ADD COLUMN content_rating TEXT;
ALTER TABLE followed_shows ADD COLUMN content_rating TEXT;
