-- The titles of each Home row that weren't on the Server at its refresh, as the title source gave
-- them (ADR 0058): a JSON array of `{"title": ["movie", "tmdb-603"], "id": "tmdb-603", "name":
-- "The Matrix", "year": 1999, "poster": ["tmdb", "/f89U3ADr1oiB1s9GkdPOEpXUk5H.jpg"]}`, `id` being
-- the one its Request is kept under. Whether each is on the Server now is read live.
ALTER TABLE home_rows ADD COLUMN missing TEXT NOT NULL DEFAULT '[]';

-- So rows stored before refresh on the next visit to home, and gain their missing titles.
DELETE FROM home_row_refreshes;
