-- The version of what the Server stores from the Provider that filled a Match, `METADATA_VERSION`
-- in `src/matching.rs` (ADR 0040). Matches stored before it are older, so a Scan fetches them again.
ALTER TABLE item_matches ADD COLUMN metadata_version INTEGER NOT NULL DEFAULT 0;
-- For an Episode, its Still: an image path from the Plugin's Provider, as `poster` is.
ALTER TABLE item_matches ADD COLUMN still TEXT;
