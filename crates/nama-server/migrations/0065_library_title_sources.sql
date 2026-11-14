-- The namespaces of a Library's title sources, in the order Matching asks them, comma-separated
-- (`tvdb,tmdb`). A namespace has no comma, and the list is only read and written whole, as
-- `requests.failure_needs` is (ADR 0042). Every Library before this searched TMDB alone.
ALTER TABLE libraries ADD COLUMN title_sources TEXT NOT NULL DEFAULT 'tmdb';
