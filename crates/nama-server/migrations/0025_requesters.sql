-- Who asked for each Request.
CREATE TABLE requesters (
    request_id INTEGER NOT NULL REFERENCES requests (id) ON DELETE CASCADE,
    user_id INTEGER NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    requested_at INTEGER NOT NULL,
    PRIMARY KEY (request_id, user_id)
) STRICT;

CREATE INDEX requesters_by_user ON requesters (user_id);
