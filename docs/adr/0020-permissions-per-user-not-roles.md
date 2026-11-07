# Each User has their own Permissions, not a Role

Replaces the Admin and Member Roles. Each User holds a set of Permissions (`MANAGE_USERS`, `REQUEST_MOVIE`, `AUTO_APPROVE_TV`, …), modelled on Seerr's so Request controls can reach parity with it, and new Users get a default set an Admin can edit. `ADMIN` passes every check. A Server always keeps at least one User with `ADMIN`. Every RPC declares the Permission it needs in one table, and an RPC missing from it is refused, so a new RPC is closed until someone opens it.

## Considered Options

- **Keep Roles, check them per RPC**: rejected. Seerr-style controls such as "may request Movies but not Shows, and Movies are approved automatically" don't fit a fixed list of Roles.
- **Roles as named, editable sets of Permissions**: rejected for now. It's a second concept to explain, and Seerr reaches the same controls with per-User sets plus a default. Can be added on top later without changing the checks.
- **Declaring each RPC's Permission as a proto method option**: rejected for now. It would keep it in the schema, but it depends on the code generators exposing method options. The Rust table has a test that every RPC is listed.

## Consequences

- On upgrade, Admins get `ADMIN` and Members get the default set.
- `Permission` is a proto enum, so clients can hide what a User can't do. Its values are API contract.
- Seerr Permissions whose concepts nama doesn't have yet (Issues, auto-request from Watchlists, 4K, blocklist) are added with those concepts.
