-- How many Accounts finished each Movie and each pair of Movies, with nothing naming an Account
-- (ADR 0093). What may be stored here: `src/viewing/AGENTS.md`.
CREATE TABLE pooled_counts (
    id uuid PRIMARY KEY DEFAULT uuidv7(),
    -- The pair's External IDs, the lesser first. A Movie's own count is its pair with itself.
    title_a text NOT NULL,
    title_b text NOT NULL CHECK (title_a <= title_b),
    accounts bigint NOT NULL DEFAULT 1,
    UNIQUE (title_a, title_b)
);

-- An Account's choice to pool; one with no row pools, as pooling is on by default (ADR 0093).
CREATE TABLE pooling_settings (
    id uuid PRIMARY KEY DEFAULT uuidv7(),
    account_id uuid NOT NULL UNIQUE REFERENCES accounts (id) ON DELETE CASCADE,
    pooling boolean NOT NULL
);

-- Whether the event added its Movie to the Account's counts, which only the Account's first
-- finished play of a Movie does. It goes with its event, so deleting an event the Account wants
-- forgotten forgets that the Movie was counted, too.
ALTER TABLE viewing_events ADD COLUMN pooled boolean NOT NULL DEFAULT false;

-- At most one counted play of a Movie per Account: two sent at once fail one batch, and the
-- Server's retry records that one uncounted.
CREATE UNIQUE INDEX viewing_events_pooled ON viewing_events (account_id, external_id) WHERE pooled;
