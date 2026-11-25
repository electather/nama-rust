-- The Server's id at the Hub, which every sign-in token the Hub signs names as its audience, so one
-- meant for another Server is refused (ADR 0105). The token a newly Linked Server gets names it as
-- its subject; a Link the Hub Plugin made learns it from the first sign-in token the Hub hands it
-- over that Link, so it stays NULL until then.
ALTER TABLE hub_link ADD COLUMN server_id TEXT;

-- Each User's Link to an Account, which the Kernel keeps now rather than the Hub Plugin (ADR
-- 0105), so Settings still shows it while the Hub is unreachable (ADR 0002).
CREATE TABLE hub_user_links (
    user_id INTEGER PRIMARY KEY NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    -- The SHA-256 of the credential of the Server's Link they Linked under, base64url without
    -- padding. Unlinking the Server Unlinks its Users on the Hub, so one kept under an earlier Link
    -- is ignored.
    server_link TEXT NOT NULL,
    -- The email of the Account they are Linked to.
    account TEXT NOT NULL
) STRICT;

-- Taken over from the Hub Plugin's Store, which kept each as JSON under `user-link/<user id>`, so
-- a Linked User stays Linked. One of a User since deleted is dropped, as its foreign key would
-- fail the upgrade.
INSERT INTO hub_user_links (user_id, server_link, account)
SELECT CAST(substr(key, 11) AS INTEGER),
       json_extract(CAST(value AS TEXT), '$.server_link'),
       json_extract(CAST(value AS TEXT), '$.account')
FROM plugin_store
WHERE plugin_id = 'hub' AND user_id IS NULL AND key LIKE 'user-link/%'
  AND json_valid(CAST(value AS TEXT))
  AND json_type(CAST(value AS TEXT), '$.server_link') = 'text'
  AND json_type(CAST(value AS TEXT), '$.account') = 'text'
  AND CAST(substr(key, 11) AS INTEGER) IN (SELECT id FROM users);

DELETE FROM plugin_store WHERE plugin_id = 'hub' AND user_id IS NULL AND key LIKE 'user-link/%';
