-- Each Account's merged Watchlist (ADR 0056): per title, the change its Linked Users' Servers made
-- last, removals included. What may be stored here: `src/sync/AGENTS.md`.
CREATE TABLE watchlists (
    id uuid PRIMARY KEY DEFAULT uuidv7(),
    account_id uuid NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,
    -- Movie and Show TMDB ids are separate id spaces, so the kind is part of the key.
    show boolean NOT NULL,
    -- The External ID a Server added it under, such as `tmdb-603` (ADR 0013), whether or not any
    -- Server has the title.
    title_id text NOT NULL,
    -- Kept once taken off, so a Server that still has it takes the removal.
    removed boolean NOT NULL,
    -- When the Server made the change, which decides whose wins.
    changed_at timestamptz NOT NULL,
    UNIQUE (account_id, show, title_id)
);
