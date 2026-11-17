-- What searching the Libraries compares: one row per Matched Item, by its id, kept current by
-- the triggers below. `remove_diacritics 2` lets "amelie" find Amélie.
CREATE VIRTUAL TABLE title_search USING fts5(
    -- The Movie's or Episode's title, then again without the marks that join words, so
    -- "spiderman" finds Spider-Man.
    title,
    -- The year it was released or first aired.
    year,
    -- For an Episode, its Show's title, as `title` is.
    show_title,
    tokenize = 'unicode61 remove_diacritics 2'
);

-- ponytail: only these marks join words; add any other a title is found missing for.
CREATE VIEW title_search_rows (item_id, title, year, show_title) AS
SELECT item_id,
       title || ' ' || replace(replace(replace(replace(replace(
           title, '-', ''), '''', ''), '’', ''), '.', ''), '·', ''),
       substr(release_date, 1, 4),
       show_title || ' ' || replace(replace(replace(replace(replace(
           show_title, '-', ''), '''', ''), '’', ''), '.', ''), '·', '')
FROM item_matches
WHERE state = 'matched';
