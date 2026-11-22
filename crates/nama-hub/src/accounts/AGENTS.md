## Accounts

Accounts (GLOSSARY.md): creating one with an email and a password, and signing it in and out with a web session.

- **One owner.** Only this area reads or writes `accounts` and `account_sessions`, tests included. Every table holding an Account's data references `accounts(id) ON DELETE CASCADE` (ADR 0099).
- **Emails compare without regard to case**, through the unique index on `lower(email)`, and are kept as typed. Nothing here proves one is real: a Sign-in Link does (ADR 0100).
- **Passwords are argon2id** with the `argon2` crate's defaults, hashed and checked on a blocking thread, at most one per core at once (`passwords.rs`). Signing in with an unknown email checks a dummy hash, so it takes as long as a wrong password.
- **Sessions are opaque.** A token is 32 random bytes, stored only as its SHA-256 hash, and ended by deleting its row (ADR 0098). It never reaches a response body (ADR 0015).
- **Rate limits live in memory** (ADR 0096): failed sign-ins per client IP, an IPv6 client by its /64, and per email, and Account creations per client IP, checked before any argon2 work (`rate_limits.rs`).

### Neighbours

- Uses: nothing but the database.
- Used by: `api`, whose auth layer turns a session cookie into an `AccountSession`, and which converts the rest of what `accounts.rs` re-exports: `Account`, `SignedIn`, `Error`, `Blocked` and the length and lifetime limits.
