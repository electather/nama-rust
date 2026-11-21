-- When each dated Episode of a Show aired, as the title source gave its Season when the Show was
-- last fetched, so a Show page counts its aired Episodes against today when it asks. Kept once per
-- Show and keyed as `title_scores` is: by the Plugin whose source gave it, since sources number
-- Episodes differently, and by the Show's first id in namespace order. An undated Episode has no
-- row, since it never counts as aired.
CREATE TABLE episode_air_dates (
    plugin TEXT NOT NULL,
    namespace TEXT NOT NULL,
    value TEXT NOT NULL,
    season INTEGER NOT NULL,
    episode INTEGER NOT NULL,
    -- `YYYY-MM-DD`, so it compares with a date as text.
    air_date TEXT NOT NULL,
    PRIMARY KEY (plugin, namespace, value, season, episode)
) STRICT, WITHOUT ROWID;
