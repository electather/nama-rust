-- Where the Hub's page sends the person once they approve a Server's code, for Linking by
-- redirect (ADR 0105), such as `https://nama.example/settings`, or empty for a code typed in.
ALTER TABLE server_link_codes
    ADD COLUMN return_url text NOT NULL DEFAULT '';
