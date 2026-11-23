-- An Episode's Show, by its External ID, such as `tmdb-1399`, so the Hub can reach it through
-- `titles`, which hold series and never Episodes (ADR 0095). Null for a Movie, and for an Episode
-- recorded before this column.
ALTER TABLE viewing_events ADD COLUMN show_external_id text;
