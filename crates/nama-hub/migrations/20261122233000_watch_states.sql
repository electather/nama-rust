-- Each Account's merged Watch State (ADR 0008): per title, the change its Linked Users' Servers
-- made last. What may be stored here: `src/sync/AGENTS.md`.
CREATE TABLE watch_states (
    id uuid PRIMARY KEY DEFAULT uuidv7(),
    account_id uuid NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,
    -- Movie and Episode TMDB ids are separate id spaces, so the kind is part of the key.
    episode boolean NOT NULL,
    -- One of the title's External IDs, such as `tmdb-348` (ADR 0013). A title known by several
    -- has a row under each, so a Server finds it under any it knows.
    title_id text NOT NULL,
    played boolean NOT NULL,
    position_ms bigint NOT NULL CHECK (position_ms BETWEEN 0 AND duration_ms),
    duration_ms bigint NOT NULL,
    -- When the Server made the change, which decides whose wins.
    changed_at timestamptz NOT NULL,
    UNIQUE (account_id, episode, title_id)
);
