-- The Library a Home row's heading opens, as its Plugin gave it, or NULL. No foreign key: a
-- Plugin's id for a Library deleted since, or never there, mustn't fail its refresh, so listing
-- the row links it only to a Library its reader sees.
ALTER TABLE home_rows ADD COLUMN library_id INTEGER;
