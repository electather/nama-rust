-- What an admin sets about the Server as a whole, in at most one row: none until it is first set.
CREATE TABLE server_settings (
    id INTEGER PRIMARY KEY NOT NULL CHECK (id = 1),
    -- The address people reach the Server at, an http or https URL, or '' for none.
    url TEXT NOT NULL
) STRICT;
