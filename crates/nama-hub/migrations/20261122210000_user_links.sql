-- Users Linked to Accounts, and the codes they wait on (ADR 0008, ADR 0011). What may be stored
-- here: `src/links/AGENTS.md`.
CREATE TABLE user_links (
    id uuid PRIMARY KEY DEFAULT uuidv7(),
    account_id uuid NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,
    linked_server_id uuid NOT NULL REFERENCES linked_servers (id) ON DELETE CASCADE,
    -- The User's id on their Server.
    server_user_id bigint NOT NULL,
    -- The User's name on their Server when they Linked.
    user_name text NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    -- A User Links to one Account.
    UNIQUE (linked_server_id, server_user_id)
);

CREATE INDEX user_links_by_account ON user_links (account_id);

-- A User waiting for someone to enter the code their Server shows. Deleted once it is entered;
-- expired ones go when the next User starts.
CREATE TABLE user_link_codes (
    id uuid PRIMARY KEY DEFAULT uuidv7(),
    linked_server_id uuid NOT NULL REFERENCES linked_servers (id) ON DELETE CASCADE,
    server_user_id bigint NOT NULL,
    user_name text NOT NULL,
    -- What a person enters, without its hyphen, such as `BDFHJKLM`.
    user_code text NOT NULL UNIQUE,
    expires_at timestamptz NOT NULL,
    -- Starting again replaces the User's code.
    UNIQUE (linked_server_id, server_user_id)
);
