-- Each Followed Show that asked for a Season Request, whether it opened or joined it, so its
-- Requesters count for that Season (#989). `requests.followed_show_id` names only the one that
-- opened it, which decides and hands it off. Ending the Followed Show stops it counting.
CREATE TABLE request_followed_shows (
    request_id INTEGER NOT NULL REFERENCES requests (id) ON DELETE CASCADE,
    followed_show_id INTEGER NOT NULL REFERENCES followed_shows (id) ON DELETE CASCADE,
    PRIMARY KEY (request_id, followed_show_id)
) STRICT, WITHOUT ROWID;

CREATE INDEX request_followed_shows_by_followed_show ON request_followed_shows (followed_show_id);

-- Only the opening Followed Show was recorded before; which ones joined a Request can't be known.
INSERT INTO request_followed_shows (request_id, followed_show_id)
SELECT id, followed_show_id FROM requests WHERE followed_show_id IS NOT NULL;
