-- A Movie's or Show's Similar titles, kept once per title like its Score (ADR 0081) and keyed as
-- `title_scores` is, so a title Matched through another source never shows this one's.
CREATE TABLE similar_titles (
    plugin TEXT NOT NULL,
    kind TEXT NOT NULL CHECK (kind IN ('movie', 'show')),
    namespace TEXT NOT NULL,
    value TEXT NOT NULL,
    -- A JSON array of their ids, such as '["tmdb-603"]', most alike first.
    similar TEXT NOT NULL,
    PRIMARY KEY (plugin, kind, namespace, value)
) STRICT, WITHOUT ROWID;
