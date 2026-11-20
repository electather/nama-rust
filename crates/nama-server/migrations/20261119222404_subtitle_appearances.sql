-- Each User's Subtitle Appearance: how big their text subtitles are and what's behind them. A User
-- who never picked has no row, and a column they never set is NULL; both read as the defaults.
-- `size` and `background` are `nama.v1.SubtitleSize` and `nama.v1.SubtitleBackground` numbers:
-- those are API contract, so they never change under stored rows. Deleting the User deletes theirs.
CREATE TABLE subtitle_appearances (
    user_id INTEGER PRIMARY KEY REFERENCES users (id) ON DELETE CASCADE,
    size INTEGER CHECK (size > 0),
    background INTEGER CHECK (background > 0)
) STRICT;
