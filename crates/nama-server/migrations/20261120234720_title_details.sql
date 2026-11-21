-- What a title source says of a whole Movie or Show, kept once per title rather than copied onto
-- each of its Matches like genres, so a Show's isn't stored on every Episode's (ADR 0081). Keyed
-- by the Plugin whose source said it, so a title Matched through another source never shows it,
-- and by the title's first id in namespace order among those its Matches store.
CREATE TABLE title_details (
    plugin TEXT NOT NULL,
    kind TEXT NOT NULL CHECK (kind IN ('movie', 'show')),
    namespace TEXT NOT NULL,
    value TEXT NOT NULL,
    -- From 0 to 10, NULL when nobody has voted.
    score REAL CHECK (score BETWEEN 0 AND 10),
    -- ISO 639-1, such as 'ja'.
    original_language TEXT,
    PRIMARY KEY (plugin, kind, namespace, value)
) STRICT, WITHOUT ROWID;
