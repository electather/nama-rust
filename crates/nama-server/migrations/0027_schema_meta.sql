-- Facts about the database itself, as key/value pairs, for the Server's own bookkeeping.
CREATE TABLE schema_meta (
    key TEXT PRIMARY KEY NOT NULL,
    value TEXT NOT NULL
) STRICT;
