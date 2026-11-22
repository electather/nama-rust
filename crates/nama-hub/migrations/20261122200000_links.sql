-- Linked Servers and the device flow that registers them (ADR 0011). What may be stored here:
-- `src/links/AGENTS.md`.
CREATE TABLE linked_servers (
    id uuid PRIMARY KEY DEFAULT uuidv7(),
    account_id uuid NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,
    -- The Server's name when it was Linked.
    name text NOT NULL,
    -- The SHA-256 hash of the Server's opaque credential (ADR 0098). NULL from the code being
    -- entered until the Server polls and collects it.
    credential_hash bytea UNIQUE,
    created_at timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX linked_servers_by_account ON linked_servers (account_id);

-- A Server waiting for someone to enter the code it shows. Deleted once the Server collects its
-- credential; expired ones go when the next Server starts.
CREATE TABLE server_link_codes (
    id uuid PRIMARY KEY DEFAULT uuidv7(),
    -- The SHA-256 hash of the device code the Server polls with.
    device_code_hash bytea NOT NULL UNIQUE,
    -- What a person enters, without its hyphen, such as `BDFHJKLM`.
    user_code text NOT NULL UNIQUE,
    server_name text NOT NULL,
    -- Set once the code was entered.
    linked_server_id uuid REFERENCES linked_servers (id) ON DELETE CASCADE,
    expires_at timestamptz NOT NULL
);
