-- A Match's titled Backdrop: the best Backdrop with its title written on it, in the language
-- artwork is picked in, for clients that show it without a Title Logo over it (#1339). For an Episode it is its Show's, as `backdrop` is.
-- `titled_backdrop_plugin` names the Plugin whose title source `image` serves it, as
-- `backdrop_plugin` does. Matches stored before stay NULL until the next scan fetches them again
-- at the new metadata version.
ALTER TABLE item_matches ADD COLUMN titled_backdrop TEXT;
ALTER TABLE item_matches ADD COLUMN titled_backdrop_plugin TEXT;
