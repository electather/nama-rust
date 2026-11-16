-- The Plugin Permissions an Admin granted each Plugin when enabling it, as a JSON array of their
-- names (ADR 0049).
ALTER TABLE plugin_settings ADD COLUMN granted_permissions TEXT NOT NULL DEFAULT '[]';
