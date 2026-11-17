-- The kinds of Request notification each User turned off, on every Notifier. Every kind is on
-- until its User turns it off, so a new User has no rows. `kind` is a `nama.v1.NotificationKind`
-- number: those are API contract, so they never change under stored rows. Deleting the User
-- deletes theirs.
CREATE TABLE notifications_off (
    user_id INTEGER NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    kind INTEGER NOT NULL CHECK (kind > 0),
    PRIMARY KEY (user_id, kind)
) STRICT;
