-- The Permissions a new User gets (ADR 0020), as the bitset `users.permissions` holds: REQUEST (6)
-- with VIEW_REQUESTS (12) is 64 + 4096, for a fresh Server and an upgraded one alike. Always one
-- row, with `id` 1.
CREATE TABLE default_permissions (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    permissions INTEGER NOT NULL
) STRICT;
INSERT INTO default_permissions (id, permissions) VALUES (1, 4160);
