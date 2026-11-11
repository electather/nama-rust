-- The Plugin whose Fulfiller a Request was handed to at its latest Hand-off, so a Failed one can
-- point at the Plugin to fix: NULL until it is handed off.
ALTER TABLE requests ADD COLUMN fulfiller_plugin_id TEXT;
