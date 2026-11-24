-- The Server's Link with the Hub, which the Kernel keeps now rather than the Hub Plugin (ADR 0105):
-- at most one row.
CREATE TABLE hub_link (
    id INTEGER PRIMARY KEY NOT NULL CHECK (id = 1),
    -- What the Server presents on every later Hub call (ADR 0098).
    credential TEXT NOT NULL,
    -- The Account's email, as the Hub's signed token named it; NULL if the token couldn't be
    -- checked when the credential came.
    account TEXT
) STRICT;

-- The Hub's key set as last fetched, so a token it signed passes while it is unreachable: at most
-- one row.
CREATE TABLE hub_key_set (
    id INTEGER PRIMARY KEY NOT NULL CHECK (id = 1),
    key_set BLOB NOT NULL
) STRICT;

-- Taken over from the Hub Plugin's Store, which kept the Link as JSON, so a Linked Server stays
-- Linked with the same credential. Its Users' Links name it by that credential, so they hold too.
INSERT INTO hub_link (id, credential, account)
SELECT 1, json_extract(CAST(value AS TEXT), '$.credential'),
       json_extract(CAST(value AS TEXT), '$.account')
FROM plugin_store
WHERE plugin_id = 'hub' AND user_id IS NULL AND key = 'link'
  AND json_valid(CAST(value AS TEXT))
  AND json_type(CAST(value AS TEXT), '$.credential') = 'text';

INSERT INTO hub_key_set (id, key_set)
SELECT 1, value FROM plugin_store WHERE plugin_id = 'hub' AND user_id IS NULL AND key = 'key-set';

DELETE FROM plugin_store WHERE plugin_id = 'hub' AND user_id IS NULL AND key IN ('link', 'key-set');
