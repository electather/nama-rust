-- An Account's passkeys: WebAuthn credentials for the Hub's public address. What may be stored
-- here: `src/accounts/AGENTS.md`.
CREATE TABLE account_passkeys (
    id uuid PRIMARY KEY DEFAULT uuidv7(),
    account_id uuid NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,
    -- The authenticator's id for it, which signing in finds it by.
    credential_id bytea NOT NULL UNIQUE,
    -- What the Account named it, such as "Phone".
    name text NOT NULL,
    -- The COSE algorithm its signatures use: -7 (ES256) or -257 (RS256).
    algorithm integer NOT NULL,
    -- A DER SubjectPublicKeyInfo, as the browser gave it.
    public_key bytea NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX account_passkeys_by_account ON account_passkeys (account_id);
