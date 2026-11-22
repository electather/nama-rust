-- Sign-in Links (GLOSSARY.md), and the proof of an Account's email that using one gives (ADR 0100).
-- What may be stored here: `src/accounts/AGENTS.md`.

-- The first Sign-in Link used on an unproven Account removes its password.
ALTER TABLE accounts ALTER COLUMN password_hash DROP NOT NULL;
ALTER TABLE accounts ADD COLUMN email_proven boolean NOT NULL DEFAULT false;

-- An Account's waiting Sign-in Link: at most one, which asking again replaces. Its token is kept
-- only as its SHA-256 hash (ADR 0098), and using it deletes its row.
CREATE TABLE sign_in_links (
    id uuid PRIMARY KEY DEFAULT uuidv7(),
    account_id uuid NOT NULL UNIQUE REFERENCES accounts (id) ON DELETE CASCADE,
    token_hash bytea NOT NULL UNIQUE,
    -- The Hub path the sign-in continues at.
    continue_to text NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now()
);
