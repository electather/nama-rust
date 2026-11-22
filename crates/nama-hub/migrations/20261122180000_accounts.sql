-- Accounts (GLOSSARY.md) and their web sessions. What may be stored here: `src/accounts/AGENTS.md`.
CREATE TABLE accounts (
    id uuid PRIMARY KEY DEFAULT uuidv7(),
    -- As it was typed. Two Accounts can't have emails that differ only in case.
    email text NOT NULL,
    -- An argon2id PHC string.
    password_hash text NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now()
);

CREATE UNIQUE INDEX accounts_by_email ON accounts (lower(email));

-- A signed-in browser's session: a random opaque token, kept only as its SHA-256 hash and ended by
-- deleting its row (ADR 0098).
CREATE TABLE account_sessions (
    id uuid PRIMARY KEY DEFAULT uuidv7(),
    account_id uuid NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,
    token_hash bytea NOT NULL UNIQUE,
    created_at timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX account_sessions_by_account ON account_sessions (account_id);
