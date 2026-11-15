## Users

Users, sign-in and their Personal settings. Only Personal settings have moved in so far.

- **One owner for Personal settings:** only `personal_settings.rs` reads or writes `personal_settings`, tests included; values are secrets in logs, and a secret never goes back to a client (ADR 0046).

### Neighbours

- Uses: `db`.
- Used by: `events` (Personal settings for Notifier deliveries) and the `api` part for Users.
