-- Libraries: named collections of one kind of media, each backed by one or more folders.
-- AUTOINCREMENT so a deleted Library's id is never handed to a new one: a client still holding
-- the old id gets `not_found` instead of a different Library.
CREATE TABLE libraries (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    kind TEXT NOT NULL CHECK (kind IN ('movies', 'shows')),
    -- The namespaces of a Library's title sources, in the order Matching asks them, comma-separated
    -- (`tvdb,tmdb`). A namespace has no comma, and the list is only read and written whole, as
    -- `requests.failure_needs` is (ADR 0042).
    title_sources TEXT NOT NULL DEFAULT 'tmdb',
    -- The ids of the Plugins whose Recognisers a Library asks, in the order it asks them,
    -- comma-separated as `title_sources` is. A Plugin id has no comma. Empty means the Kernel's own
    -- Matching alone (ADR 0050).
    recognisers TEXT NOT NULL DEFAULT '',
    -- The Recognisers a Library listed that were enabled, in order and comma-separated as
    -- `recognisers` is, when its last scan started. A scan that finds them changed reads the Library's
    -- unparsed and Unmatched files again (ADR 0050). Empty matches a Library listing none.
    recognisers_read TEXT NOT NULL DEFAULT ''
) STRICT;

-- A Library holds one kind of media for its whole life.
CREATE TRIGGER libraries_kind_is_fixed
BEFORE UPDATE OF kind ON libraries
WHEN NEW.kind IS NOT OLD.kind
BEGIN
    SELECT RAISE(ABORT, 'a Library''s kind can''t change');
END;
