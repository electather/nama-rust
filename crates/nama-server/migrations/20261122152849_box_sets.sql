-- A Movie's Box Set (`GLOSSARY.md`), kept once per title beside its Score (ADR 0081), as the Box
-- Set's External ID, such as 'tmdb-8091'. NULL for a Movie in none, and for a Show.
ALTER TABLE title_details ADD COLUMN box_set TEXT;

-- Each Box Set as the Movie of it fetched last gave it, keyed on its External ID alone: Box Sets
-- of different title sources keep titles under different namespaces, so they never share a row.
-- A row outlives its Movies' Matches, as other per-title data does; reads show only a Box Set
-- with a Matched Movie.
CREATE TABLE box_sets (
    namespace TEXT NOT NULL,
    value TEXT NOT NULL,
    -- The Plugin whose title source gave it, which serves its images.
    plugin TEXT NOT NULL,
    name TEXT NOT NULL,
    overview TEXT,
    poster TEXT,
    backdrop TEXT,
    -- A JSON array of its Movies' ids in release order, such as '["tmdb-348"]'; empty when its
    -- title source doesn't list them.
    movies TEXT NOT NULL,
    -- A JSON array of them as the title source had them when last looked up, which shows those
    -- the Server lacks: '[{"listed_as":"tmdb-679","id":"tmdb-679","title":"Aliens","year":1986,
    -- "poster":"/r1x5.jpg"}]'.
    looked_up TEXT NOT NULL,
    PRIMARY KEY (namespace, value)
) STRICT, WITHOUT ROWID;
