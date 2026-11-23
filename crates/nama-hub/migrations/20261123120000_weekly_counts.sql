-- How many Accounts finished each Movie each week, which charts such as what's trending read, with
-- nothing naming an Account (ADR 0093). What may be stored here: `src/viewing/AGENTS.md`.
CREATE TABLE weekly_counts (
    -- Random, not `uuidv7()`: why is in `crates/nama-hub/AGENTS.md`.
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    -- The Monday, in UTC, of the week the plays started in.
    week date NOT NULL,
    title text NOT NULL,
    accounts bigint NOT NULL DEFAULT 1,
    UNIQUE (week, title)
);

-- Whether the event added its Movie to its week's count, which only the Account's first finished
-- play of that Movie in that week does. It goes with its event, as `pooled` does.
ALTER TABLE viewing_events ADD COLUMN pooled_week boolean NOT NULL DEFAULT false;

-- At most one counted play of a Movie per Account and week. `AT TIME ZONE 'UTC'` makes the week
-- immutable, as an index needs, where the session's time zone would not.
CREATE UNIQUE INDEX viewing_events_pooled_week ON viewing_events
    (account_id, external_id, date_trunc('week', started_at AT TIME ZONE 'UTC'))
    WHERE pooled_week;
