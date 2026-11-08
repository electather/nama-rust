-- A Season Request's Show's other ids, as the metadata Provider gave them when the Request was
-- opened, for the Fulfiller for Shows: Sonarr finds a Show by its TVDB id. NULL for a Movie, or
-- where the Provider had none.
ALTER TABLE requests ADD COLUMN show_imdb_id TEXT;
ALTER TABLE requests ADD COLUMN show_tvdb_id INTEGER;
