-- A way a User proves who they are. Plugins have no storage, so the Kernel keeps the credential
-- for the Plugin `provider_id`, which alone can make and check it (for `local-auth`, an argon2id
-- hash).
CREATE TABLE sign_in_methods (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    provider_id TEXT NOT NULL,
    -- NULL for a password; otherwise who the person is to the provider, by which the User is found.
    -- UNIQUE treats NULLs as distinct, so this constrains only the non-passwords.
    subject TEXT,
    credential TEXT NOT NULL,
    -- What the User is shown, such as "Password".
    name TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    UNIQUE (provider_id, subject)
) STRICT;

CREATE INDEX sign_in_methods_by_user ON sign_in_methods (user_id);

-- At most one password per User. Changing it updates this row, so the Devices that signed in with
-- it stay signed in.
CREATE UNIQUE INDEX one_password_per_user ON sign_in_methods (user_id) WHERE subject IS NULL;

-- A User always keeps a way in. Deleting the User, which cascades here, is still allowed.
CREATE TRIGGER sign_in_methods_keep_one BEFORE DELETE ON sign_in_methods
WHEN NOT EXISTS (SELECT 1 FROM sign_in_methods WHERE user_id = OLD.user_id AND id != OLD.id)
    AND EXISTS (SELECT 1 FROM users WHERE id = OLD.user_id)
BEGIN
    SELECT RAISE(ABORT, 'a User must keep at least one Sign-in Method');
END;
