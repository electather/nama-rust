-- The ids of the Plugins whose Recognisers a Library asks, in the order it asks them,
-- comma-separated as `title_sources` is (migration 0065). A Plugin id has no comma. Empty, as
-- every Library before this is, means the Kernel's own Matching alone (ADR 0050).
ALTER TABLE libraries ADD COLUMN recognisers TEXT NOT NULL DEFAULT '';
