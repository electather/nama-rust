-- A Match's ids are read from `item_match_ids` alone now (ADR 0042). ALTER rather than a rebuild,
-- so columns other migrations add to `item_matches` are kept.
ALTER TABLE item_matches DROP COLUMN tmdb_id;
ALTER TABLE item_matches DROP COLUMN imdb_id;
ALTER TABLE item_matches DROP COLUMN tvdb_id;
ALTER TABLE item_matches DROP COLUMN show_tmdb_id;
ALTER TABLE item_matches DROP COLUMN show_imdb_id;
ALTER TABLE item_matches DROP COLUMN show_tvdb_id;
