-- A Match's Title Logo and the Plugin whose title source `image` serves it, set together. For an
-- Episode they hold its Show's logo, as `backdrop` holds its Show's backdrop.
ALTER TABLE item_matches ADD COLUMN logo TEXT;
ALTER TABLE item_matches ADD COLUMN logo_plugin TEXT;
