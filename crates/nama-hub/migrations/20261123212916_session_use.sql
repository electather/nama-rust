-- Where and when each session was last used, which the Account page lists. What may be stored
-- here: `src/accounts/AGENTS.md`.
ALTER TABLE account_sessions
    -- As the browser last sent it, cut to 512 characters, or NULL until it sends one.
    ADD COLUMN user_agent text,
    -- Kept to within 5 minutes, so not every request writes it.
    ADD COLUMN last_used_at timestamptz NOT NULL DEFAULT now();

UPDATE account_sessions SET last_used_at = created_at;
