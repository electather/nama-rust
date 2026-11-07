-- Permissions replace Roles (ADR 0020). `permissions` is a bitset: bit n is the `nama.v1.Permission`
-- numbered n, so ADMIN (1) is 2, and REQUEST (6) with VIEW_REQUESTS (12) is 64 + 4096. The column
-- needs a default to be added; every insert sets it.
ALTER TABLE users ADD COLUMN permissions INTEGER NOT NULL DEFAULT 0;
UPDATE users SET permissions = CASE role WHEN 'admin' THEN 2 ELSE 4160 END;
ALTER TABLE users DROP COLUMN role;
