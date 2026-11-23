-- An Account Links at most one User on each Server: signing in to that Server with the Account
-- names one User, and the Hub keeps one Watch State per Account, which two Users would overwrite
-- (#2045). Of the Users an Account Linked on one Server already, the one Linked first stays Linked,
-- as signing in already chose it.
DELETE FROM user_links WHERE id IN (
    SELECT id FROM (
        SELECT id, row_number() OVER (
            PARTITION BY account_id, linked_server_id ORDER BY created_at, id
        ) AS nth
        FROM user_links
    ) AS ranked
    WHERE nth > 1
);

ALTER TABLE user_links ADD CONSTRAINT user_links_one_per_account_and_server
    UNIQUE (account_id, linked_server_id);

-- The constraint's index starts with `account_id`, so it serves this one's lookups.
DROP INDEX user_links_by_account;
