-- Each of a Match's `genres`' id in its title source, in the same order, as a JSON array of their
-- text forms such as "tmdb-35"; NULL or empty where the source gave none (#1718).
ALTER TABLE item_matches ADD COLUMN genre_ids TEXT;
