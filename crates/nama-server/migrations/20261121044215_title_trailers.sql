-- ADR 0081's per-title store now keeps a title's Trailers beside its Score, so it is named for
-- neither.
ALTER TABLE title_scores RENAME TO title_details;

-- A Movie's or Show's Trailers, at most 5, as a JSON array of `{"name": ..., "url": ...}` in the
-- order to show them. A title stored before has none until its Matches are refreshed.
ALTER TABLE title_details ADD COLUMN trailers TEXT NOT NULL DEFAULT '[]';
