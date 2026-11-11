-- Each User's Track Preferences: what they want a kind of Track to start as when they play
-- something (ADR 0035). Kept against the Show's or Movie's external id, as Watch State is (ADR
-- 0013), never against an Item or file.
CREATE TABLE track_preferences (
    user_id INTEGER NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    -- Whose Preference it is: a Show's or a Movie's, or the User's own for everything they play.
    scope TEXT NOT NULL CHECK (scope IN ('user', 'show', 'movie')),
    -- '' for the User's own, else the external id, as `tmdb-348`, browsing identifies it by.
    title_id TEXT NOT NULL CHECK ((scope = 'user') = (title_id = '')),
    kind TEXT NOT NULL CHECK (kind IN ('subtitle')),
    -- Set when the User turned this kind off, and then nothing describes a Track.
    off INTEGER NOT NULL CHECK (off IN (0, 1)),
    -- What describes the Track picked, as the file tagged it: not its place, since each Episode is
    -- its own file, nor its format, since a Device decides whether it draws it.
    language TEXT,
    forced INTEGER NOT NULL CHECK (forced IN (0, 1)),
    sdh INTEGER NOT NULL CHECK (sdh IN (0, 1)),
    title TEXT,
    PRIMARY KEY (user_id, scope, title_id, kind)
) STRICT;
