-- The Recognisers a Library listed that were enabled, in order and comma-separated as
-- `recognisers` is, when its last scan started. A scan that finds them changed reads the Library's
-- unparsed and Unmatched files again (ADR 0050). Empty, as every Library before this is, matches a
-- Library listing none.
ALTER TABLE libraries ADD COLUMN recognisers_read TEXT NOT NULL DEFAULT '';
