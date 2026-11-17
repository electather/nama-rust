-- Who asked for each Followed Show. The Season Requests it opens later get these as Requesters.
CREATE TABLE followed_show_requesters (
    followed_show_id INTEGER NOT NULL REFERENCES followed_shows (id) ON DELETE CASCADE,
    user_id INTEGER NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    requested_at INTEGER NOT NULL,
    PRIMARY KEY (followed_show_id, user_id)
) STRICT;

CREATE INDEX followed_show_requesters_by_user ON followed_show_requesters (user_id);
