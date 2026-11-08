-- A Request asks for one Movie, or one Season of a Show (ADR 0012). For a Season, `season` is its
-- number, `tmdb_id` and `title` are the Show's, and `release_date` is the Season's first air date.
ALTER TABLE requests ADD COLUMN season INTEGER CHECK (season >= 0);

-- A Movie, or a Season, has at most one open Request. TMDB numbers Movies and Shows apart, so a
-- Movie and a Show may share a `tmdb_id`; -1 stands for a Movie's missing Season, since a unique
-- index counts every NULL as distinct.
DROP INDEX one_open_request_per_movie;
CREATE UNIQUE INDEX one_open_request_per_movie_or_season ON requests (tmdb_id, IFNULL(season, -1))
WHERE state NOT IN ('declined', 'available');
