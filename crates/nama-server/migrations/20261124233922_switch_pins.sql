-- Each User's Switch PIN, which a Device asks for before switching to them (ADR 0106), as an
-- argon2id PHC string. A User without a row has none.
CREATE TABLE switch_pins (
    user_id INTEGER PRIMARY KEY REFERENCES users (id) ON DELETE CASCADE,
    credential TEXT NOT NULL
) STRICT;
