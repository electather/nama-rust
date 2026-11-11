-- What searching the Libraries compares: one row per Matched item, by item id, kept current by
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

-- shortcut: only these marks join words; add any other a title is found missing for.
CREATE VIEW title_search_rows (item_id, title, year, show_title) AS
SELECT item_id,
       title || ' ' || replace(replace(replace(replace(replace(
           title, '-', ''), '''', ''), '’', ''), '.', ''), '·', ''),
       substr(release_date, 1, 4),
       show_title || ' ' || replace(replace(replace(replace(replace(
           show_title, '-', ''), '''', ''), '’', ''), '.', ''), '·', '')
FROM item_matches
WHERE state = 'matched';

-- `INSERT OR REPLACE` fires no delete trigger while `recursive_triggers` is off, so this drops
-- the row a re-Match replaces. Nothing updates `item_matches`; one that does needs a trigger too.
CREATE TRIGGER item_matches_searched AFTER INSERT ON item_matches
BEGIN
    DELETE FROM title_search WHERE rowid = NEW.item_id;
    INSERT INTO title_search (rowid, title, year, show_title)
    SELECT * FROM title_search_rows WHERE item_id = NEW.item_id;
END;

-- Also fires for the rows that removing a file, an item or a Library deletes by cascade.
CREATE TRIGGER item_matches_unsearched AFTER DELETE ON item_matches
BEGIN
    DELETE FROM title_search WHERE rowid = OLD.item_id;
END;

INSERT INTO title_search (rowid, title, year, show_title) SELECT * FROM title_search_rows;
