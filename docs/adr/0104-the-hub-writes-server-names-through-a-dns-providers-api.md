# The Hub writes Server Names through a DNS provider's API

Each Linked Server gets its own names in the zone ADR 0057 sets up (#1829). Either the Hub answers the zone's DNS queries itself, computing each answer the way plex.direct's servers do, or a DNS provider serves the zone and the Hub writes its records through the provider's API. We chose the provider, Cloudflare to start. The Hub reaches it through a trait in `dns.rs`, so another provider needs only a new adapter.

A Server's own name is its Linked Server id in hex, such as `0193b6f2c4d87a1e9f3c5b2a1d0e4f67.namadirect.net`. It never changes, so the Hub stores nothing for it. It points at the IP address the Server's reports come from, but only while the Hub can reach the Server's public port there. Each private IPv4 address among the Server's LAN addresses gets a name under it that carries the address, such as `192-168-1-10.<its own name>`. Each time the Server reports its addresses, the Hub replaces its records. The Hub removes them before it Unlinks the Server, and after it deletes the Account that owns it.

## Considered Options

- **The Hub serves the zone**: rejected. The Hub runs as one instance, and a deploy takes it down briefly (ADR 0096), but a zone needs at least two nameservers that stay up. While the Hub was down, names at home would stop resolving, yet ADR 0057 says the LAN keeps working. It would also add a DNS server to the Hub, listening on port 53 over UDP and TCP, with a database lookup for each query. In its favour, it would need no stored record for each LAN address and would have no cap on records.

## Consequences

- Every name that carries a LAN address is a stored record, and Cloudflare caps a zone's records by plan. The plan therefore limits how many Servers the zone can hold. If the Hub outgrows it, it moves to another provider, or to serving the zone itself, behind the same trait.
- A Hub with no provider set up names no Server, just as a Hub without mail sends no Sign-in Links.
- A report waits for the provider and fails if the provider is down, so the Server reports again. Unlinking also fails until the names are removed, so the Server stays Linked meanwhile.
- The names keep resolving while the Hub is down. Renewing certificates still needs the Hub (ADR 0057).
- Only private IPv4 addresses get a name, so a Server can't point one of its names at someone else's public host. IPv6 LAN addresses get no name yet.
- The id is a uuidv7, so the name shows when the Server was Linked. Its certificate's entry in the Certificate Transparency logs shows about the same.
