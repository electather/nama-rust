-- The ids (`jti`) of the tokens the web app at the Hub traded here for a Device (ADR 0105), each
-- kept until it expires, so none is traded twice.
CREATE TABLE hub_tokens_used (
    jti TEXT PRIMARY KEY NOT NULL,
    -- In seconds since the Unix epoch, past which the token no longer passes anyway.
    expires_at INTEGER NOT NULL
) STRICT;
