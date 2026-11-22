-- The Server Addresses each Linked Server last reported (`src/links/AGENTS.md`).
ALTER TABLE linked_servers
    -- The Server URL its Admin set, or empty for none.
    ADD COLUMN server_url text NOT NULL DEFAULT '',
    -- Its LAN addresses, such as `http://192.168.1.10:7980`.
    ADD COLUMN lan_addresses text[] NOT NULL DEFAULT '{}';
