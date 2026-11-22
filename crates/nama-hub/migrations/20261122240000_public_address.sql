-- Where the Hub last reached each Linked Server from outside its LAN (`src/links/AGENTS.md`).
ALTER TABLE linked_servers
    -- Such as `http://203.0.113.7:7980`, or empty while the Hub couldn't reach it.
    ADD COLUMN public_address text NOT NULL DEFAULT '';
