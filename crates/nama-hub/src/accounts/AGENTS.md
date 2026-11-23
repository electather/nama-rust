## Accounts

Accounts (GLOSSARY.md): creating one with an email and a password, signing it in, with its password, a passkey or a Sign-in Link, and out with a web session, setting its password, its passkeys, and deleting it.

- **One owner.** Only this area reads or writes `accounts`, `account_sessions`, `account_passkeys` and `sign_in_links`, tests included. Every table holding an Account's data references `accounts(id) ON DELETE CASCADE` (ADR 0099).
- **Emails compare without regard to case**, through the unique index on `lower(email)`, and are kept as typed. Only using a Sign-in Link proves one is real, and the first used on an unproven Account takes it over: it ends the other sessions and removes the password and passkeys (ADR 0100).
- **Passwords are argon2id** with the `argon2` crate's defaults, hashed and checked on a blocking thread, at most one per core at once (`passwords.rs`). Signing in with an unknown email, or one whose Account has no password, checks a dummy hash, so it takes as long as a wrong password.
- **Sessions and Sign-in Links are opaque.** A token is 32 random bytes, stored only as its SHA-256 hash, and ended by deleting its row (ADR 0098). A session token never reaches a response body (ADR 0015).
- **Recent Authentication is a session's first 10 minutes**, since every session starts at a sign-in (`sessions.rs`). Setting a password needs it, checked before anything in the request and again under the Account's row lock, as a Sign-in Link's takeover takes that lock too, and ends the Account's other sessions. Deleting the Account, and adding or removing a passkey, need it too, checked under that lock.
- **Sign-in Links reveal nothing.** Asking for one answers the same whether or not the email has an Account, and one task sends them in order, off the request path (`sign_in_links.rs`). A link lasts 15 minutes, works once, and continues only at a Hub path.
- **Passkeys are WebAuthn** for the host of `NAMA_HUB_PUBLIC_URL`, ES256 or RS256, with no attestation, checked with `ring` in `webauthn.rs`; their challenges wait in memory too (ADR 0096).
- **An Account keeps a way to sign in**: a password, a passkey, or, while mail is set up, the Sign-in Link (ADR 0100). Removing a passkey checks it under the Account's row lock.
- **Rate limits live in memory** (ADR 0096): failed sign-ins and Sign-in Links asked for, per client IP, an IPv6 client by its /64, and per email, and Account creations per client IP, checked before any argon2 work (`rate_limits.rs`).

### Neighbours

- Uses: the database, and `mail` to send Sign-in Links.
- Used by: `links`, which reads an `AccountSession`'s proven email and Recent Authentication and an Account's email, and `api`, whose auth layer turns a session cookie into an `AccountSession`, and which converts the rest of what `accounts.rs` re-exports: `Account`, `SignedIn`, `SignInLink`, `SignedInByLink`, `Error`, `Blocked`, the passkey types and the length and lifetime limits, and which builds each request's `Caller`.
