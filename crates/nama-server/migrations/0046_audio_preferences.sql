-- An Audio Preference is another kind in `track_preferences` (ADR 0035). SQLite can't change a
-- CHECK in place, so the table is rebuilt with `audio` in `kind`. Nothing references it, so
-- dropping it is safe.
CREATE TABLE track_preferences_new (
    user_id INTEGER NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    -- Whose Preference it is: a Show's or a Movie's, or the User's own for everything they play.
    scope TEXT NOT NULL CHECK (scope IN ('user', 'show', 'movie')),
    -- '' for the User's own, else the external id, as `tmdb-348`, browsing identifies it by.
    title_id TEXT NOT NULL CHECK ((scope = 'user') = (title_id = '')),
    kind TEXT NOT NULL CHECK (kind IN ('subtitle', 'audio')),
    -- Set when the User turned this kind off, and then nothing describes a Track.
    off INTEGER NOT NULL CHECK (off IN (0, 1)),
    -- What describes the Track picked, as the file tagged it: not its place, since each Episode is
    -- its own file, nor its format, since a Device decides whether it draws it. Audio is described
    -- by its language and title alone, and is never off.
    language TEXT,
    forced INTEGER NOT NULL CHECK (forced IN (0, 1)),
    sdh INTEGER NOT NULL CHECK (sdh IN (0, 1)),
    title TEXT,
    CHECK (kind = 'subtitle' OR (off = 0 AND forced = 0 AND sdh = 0)),
    PRIMARY KEY (user_id, scope, title_id, kind)
) STRICT;

INSERT INTO track_preferences_new SELECT * FROM track_preferences;
DROP TABLE track_preferences;
ALTER TABLE track_preferences_new RENAME TO track_preferences;
