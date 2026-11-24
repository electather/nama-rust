-- Where clients reach each Linked Server through the Relay it dials out to (ADR 0057).
ALTER TABLE linked_servers
    -- Such as `https://relay.<own name>:443`, or empty without a Relay.
    ADD COLUMN relay_address text NOT NULL DEFAULT '';
