-- The Posters and Backdrops admins uploaded for titles, each shown in place of the downloaded one
-- (ADR 0036). Kept against the title's external id, as Watch State is (ADR 0013), never an Item, so
-- one outlives its title's files. The bytes are in the image store, under `custom-images/`.
CREATE TABLE custom_images (
    -- Which image of which title it stands in for, as its paths name it: `movie/tmdb-348/poster`,
    -- `show/tmdb-1399/backdrop` or `show/tmdb-1399/season/2/poster`.
    slot TEXT PRIMARY KEY,
    -- Taken from the bytes, so its URL changes whenever it does.
    version TEXT NOT NULL,
    content_type TEXT NOT NULL CHECK (content_type IN ('image/jpeg', 'image/png', 'image/webp'))
) STRICT;
