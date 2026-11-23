-- Watchlist sync across a User's Links (ADR 0056): when each title was put on or taken off, and
-- what was taken off, so the change made last wins across Servers. One on before this changed at
-- 0, before any change made since.
ALTER TABLE watchlist ADD COLUMN changed_at_ms INTEGER NOT NULL DEFAULT 0;
ALTER TABLE watchlist ADD COLUMN removed INTEGER NOT NULL DEFAULT 0 CHECK (removed IN (0, 1));
