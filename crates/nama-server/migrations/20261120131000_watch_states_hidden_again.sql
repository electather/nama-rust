-- Removing a title from recently watched is back (#1155): a User's Watch State again holds
-- whether it's hidden from Continue Watching, as 0035 defined it. Hides dropped before are gone.
ALTER TABLE watch_states ADD COLUMN hidden INTEGER NOT NULL DEFAULT 0 CHECK (hidden IN (0, 1));
