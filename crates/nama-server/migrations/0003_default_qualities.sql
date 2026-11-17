-- Each User's own default Quality per Fulfiller, keyed by Plugin like `qualities` (ADR 0025). No
-- foreign key to `qualities`: replacing a list deletes its rows, and a default the admin removed
-- must stay so the User's settings can flag it. Deleting the User deletes theirs.
CREATE TABLE default_qualities (
    user_id INTEGER NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    plugin_id TEXT NOT NULL,
    quality_id INTEGER NOT NULL,
    PRIMARY KEY (user_id, plugin_id)
) STRICT;
