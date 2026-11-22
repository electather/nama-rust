-- When a Sign-in Link first proved the Account owns its email, which Linking needs (ADR 0100).
-- NULL until then. What may be stored here: `src/accounts/AGENTS.md`.
ALTER TABLE accounts ADD COLUMN email_proven_at timestamptz;
