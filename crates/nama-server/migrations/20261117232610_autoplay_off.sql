-- The Users who turned off playing the next Episode by itself. It's on until they do, so a new
-- User has no row. Deleting the User deletes theirs.
CREATE TABLE autoplay_off (
    user_id INTEGER PRIMARY KEY REFERENCES users (id) ON DELETE CASCADE
) STRICT;
