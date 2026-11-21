-- The words `title_search` holds, each with how many rows hold it, so a search can correct a
-- word no title has to the nearest one some title does (#1486). FTS5 keeps it current itself.
CREATE VIRTUAL TABLE title_search_vocab USING fts5vocab(title_search, 'row');
