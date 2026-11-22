-- Each play by a Linked User, kept under their Account for as long as it exists (ADR 0093). What
-- may be stored here: `src/viewing/AGENTS.md`.
CREATE TABLE viewing_events (
    id uuid PRIMARY KEY DEFAULT uuidv7(),
    account_id uuid NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,
    -- What the Server named the event by, so one it sends again is recorded once.
    server_key text NOT NULL,
    kind text NOT NULL CHECK (kind IN ('movie', 'episode')),
    -- The External ID the Server keeps its Watch State under, such as `tmdb-603`.
    external_id text NOT NULL,
    started_at timestamptz NOT NULL,
    -- In the Server's time zone, which `started_at` can't tell.
    time_of_day_minutes smallint NOT NULL CHECK (time_of_day_minutes BETWEEN 0 AND 1439),
    stop_position_ms bigint NOT NULL,
    duration_ms bigint NOT NULL,
    finished boolean NOT NULL,
    device_kind text NOT NULL
        CHECK (device_kind IN ('unknown', 'computer', 'phone', 'tablet', 'tv')),
    -- Also what an Account's queries go by.
    UNIQUE (account_id, server_key)
);
