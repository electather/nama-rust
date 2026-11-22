-- Users an Admin created from an Account, waiting for it to accept them (ADR 0092). Accepting
-- Links the User and deletes the row. What may be stored here: `src/links/AGENTS.md`.
CREATE TABLE user_invites (
    id uuid PRIMARY KEY DEFAULT uuidv7(),
    account_id uuid NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,
    linked_server_id uuid NOT NULL REFERENCES linked_servers (id) ON DELETE CASCADE,
    -- The User's id on their Server.
    server_user_id bigint NOT NULL,
    -- The User's name on their Server when they were created.
    user_name text NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    -- Asking again for the same User replaces the request.
    UNIQUE (linked_server_id, server_user_id)
);

CREATE INDEX user_invites_by_account ON user_invites (account_id);

-- Someone signing in to a Linked Server with their Account, as a device flow (ADR 0011): the
-- Server polls with the device code, and the Account approves the sign-in on its page by `id`.
-- Deleted once the Server collects it; expired ones go when the next starts.
CREATE TABLE server_sign_ins (
    id uuid PRIMARY KEY DEFAULT uuidv7(),
    linked_server_id uuid NOT NULL REFERENCES linked_servers (id) ON DELETE CASCADE,
    device_code_hash bytea NOT NULL UNIQUE,
    -- Where the Hub's page sends the person once they approve.
    return_url text NOT NULL,
    -- The Link of the User it signs in as, once the Account approved it. Unlinking them meanwhile
    -- takes the sign-in along.
    user_link_id uuid REFERENCES user_links (id) ON DELETE CASCADE,
    expires_at timestamptz NOT NULL
);
