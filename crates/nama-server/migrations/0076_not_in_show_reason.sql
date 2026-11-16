-- Lets `unmatched_reason` hold `not_in_show`, for an Episode whose Show has no such Season or
-- Episode. SQLite can't change a CHECK in place, and rebuilding `item_matches` would cascade into
-- `item_match_ids`, so the values move to a new column with the wider CHECK.
ALTER TABLE item_matches ADD COLUMN widened_reason TEXT
    CHECK (widened_reason IN ('ambiguous', 'no_title_fits', 'unknown_id_hint', 'not_in_show'));
UPDATE item_matches SET widened_reason = unmatched_reason;
ALTER TABLE item_matches DROP COLUMN unmatched_reason;
ALTER TABLE item_matches RENAME COLUMN widened_reason TO unmatched_reason;
