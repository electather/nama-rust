-- A Movie's or Show's Score, kept once per title rather than copied onto each of its Matches like
-- genres, so a Show's isn't stored on every Episode's (ADR 0081). Keyed by the Plugin whose source
-- gave it, so a title Matched through another source never shows it, and by the title's first id
-- in namespace order among those its Matches store. A row outlives its Matches, as other per-title
-- data does.
CREATE TABLE title_scores (
    plugin TEXT NOT NULL,
    kind TEXT NOT NULL CHECK (kind IN ('movie', 'show')),
    namespace TEXT NOT NULL,
    value TEXT NOT NULL,
    -- From 0 to 10, NULL when nobody has voted.
    score REAL CHECK (score BETWEEN 0 AND 10),
    PRIMARY KEY (plugin, kind, namespace, value)
) STRICT, WITHOUT ROWID;

-- The ISO 639-1 code of the language a Movie, or an Episode's Show, was made in, such as 'ja'.
ALTER TABLE item_matches ADD COLUMN original_language TEXT;
