-- A User who sees only a chosen list of Libraries may hold `PERMISSION_MANAGE_USERS` (bit 4) only
-- while `PERMISSION_MANAGE_LIBRARIES` (bit 8) or `PERMISSION_ADMIN` (bit 2) lets them see every one
-- (#2323), so it is taken from those who hold it without. That narrows what they may do, never
-- what they see. Their Invites go with it, as when an Admin takes it away.
DELETE FROM invites WHERE created_by IN (
    SELECT id FROM users WHERE NOT all_libraries AND permissions & 4 != 0 AND permissions & 10 = 0
);

UPDATE users SET permissions = permissions & ~4, updated_at = unixepoch()
WHERE NOT all_libraries AND permissions & 4 != 0 AND permissions & 10 = 0;
