-- When each User last opened home, so Upkeep can forget the Home rows and Featured suggestions of
-- those who stopped (#1455). Every User is counted as visiting now, so an upgrade forgets nobody's
-- rows before they could have stayed away that long.
CREATE TABLE home_visits (
    user_id INTEGER PRIMARY KEY REFERENCES users (id) ON DELETE CASCADE,
    visited_at INTEGER NOT NULL
) STRICT;

INSERT INTO home_visits (user_id, visited_at) SELECT id, unixepoch() FROM users;
