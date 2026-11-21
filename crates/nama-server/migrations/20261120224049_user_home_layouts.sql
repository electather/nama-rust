-- Each User's Home layout as JSON: the Home rows, and whether Featured, they show other than the
-- default Home layout does (ADR 0077). A User with no row follows the default. Deleting the User
-- deletes theirs.
CREATE TABLE user_home_layouts (
    user_id INTEGER PRIMARY KEY REFERENCES users (id) ON DELETE CASCADE,
    layout TEXT NOT NULL
) STRICT;
