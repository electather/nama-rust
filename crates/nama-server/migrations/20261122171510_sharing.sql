-- Whether the admin shares Diagnostics, and separately Usage Analytics, with Nama's developers
-- through the Hub: both off until they turn them on (ADR 0054, #1817).
ALTER TABLE server_settings ADD COLUMN share_diagnostics INTEGER NOT NULL DEFAULT 0;
ALTER TABLE server_settings ADD COLUMN share_usage_analytics INTEGER NOT NULL DEFAULT 0;
