-- Why automatic Matching left an item Unmatched, which says how an Admin fixes it: NULL for a
-- Match, a pin, or another reason, such as an Episode its Show doesn't have.
ALTER TABLE item_matches ADD COLUMN unmatched_reason TEXT
    CHECK (unmatched_reason IN ('ambiguous', 'no_title_fits', 'unknown_id_hint'));
-- For an ambiguous item, the titles that tied, best fit first, as a JSON array of
-- `[id, title, original_title, year]`, with the id written as an ID Hint is, such as `tmdb-2316`.
ALTER TABLE item_matches ADD COLUMN tied_titles TEXT;
