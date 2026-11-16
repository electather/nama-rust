-- The Automation reads every User's Watch State of one title under any of its ids (ADR 0048), which
-- the primary key, led by `user_id`, can't serve.
CREATE INDEX watch_states_by_title ON watch_states (kind, title_id);
