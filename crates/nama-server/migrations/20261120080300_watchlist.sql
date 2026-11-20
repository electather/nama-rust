-- Each User's Watchlist (ADR 0056): the Movies and Shows they mean to watch, kept by external id
-- whether or not the Server has them. Deleting the User deletes theirs.
CREATE TABLE watchlist (
    user_id INTEGER NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    -- Movie and Show TMDB ids are separate id spaces, so the kind is part of the key.
    kind TEXT NOT NULL CHECK (kind IN ('movie', 'show')),
    -- The id it was added under, as `tmdb-603`, `imdb-tt0133093` or `tvdb-81189`.
    title_id TEXT NOT NULL,
    PRIMARY KEY (user_id, kind, title_id)
) STRICT;
