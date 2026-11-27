-- A Linked Server's Relay as it reported it, `<ip>:<port>`, or empty without one: the Hub makes
-- the Relay's address from it when it reads it, as it does its LAN names' (#2533). The address
-- stored before can't be turned back into the Relay, so a Server has none until it next reports.
ALTER TABLE linked_servers
    DROP COLUMN relay_address,
    ADD COLUMN relay text NOT NULL DEFAULT '';
