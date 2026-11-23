-- Each Account's merged Subtitle and Audio Preferences (ADR 0035, ADR 0092): per Preference, the
-- change its Linked Users' Servers made last. What may be stored here: `src/sync/AGENTS.md`.
CREATE TABLE track_preferences (
    id uuid PRIMARY KEY DEFAULT uuidv7(),
    account_id uuid NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,
    -- An Audio Preference rather than a Subtitle Preference.
    audio boolean NOT NULL,
    -- Whose it is: a Movie's or a Show's, whose TMDB ids are separate id spaces, or the User's
    -- own for everything they play.
    scope text NOT NULL CHECK (scope IN ('user', 'movie', 'show')),
    -- The Movie's or Show's External ID, such as `tmdb-348` (ADR 0013); '' for the User's own.
    title_id text NOT NULL CHECK ((scope = 'user') = (title_id = '')),
    -- Subtitles off, or the User's own audio left to each file's. Then nothing describes a Track.
    off boolean NOT NULL,
    -- What describes the Track wanted, as its file tagged it, never which Track it was, since
    -- each Server matches it against its own: '' for no language or title.
    language text NOT NULL,
    forced boolean NOT NULL,
    sdh boolean NOT NULL,
    title text NOT NULL,
    foreign_audio_only boolean NOT NULL,
    -- When the Server made the change, which decides whose wins.
    changed_at timestamptz NOT NULL,
    UNIQUE (account_id, audio, scope, title_id)
);
