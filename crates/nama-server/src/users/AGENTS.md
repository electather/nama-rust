## Users

Users, Devices and their sessions, signing in (`sign_in.rs`, with Sign-in Codes in `sign_in_codes.rs`, the Setup Code and random codes in `codes.rs`, the sign-in limits in `rate_limits.rs` and Device names in `device_names.rs`), the session layer over HTTP (`sessions.rs`) and Personal settings.

- **Guessable requests go through `SignIn` (`sign_in.rs`).** It lets an attempt through before reading a User or a secret, and un-counts it the moment the secret checks out; a failure after that, or an input check before it, is no guess. Sign-in Codes count only an unknown or expired code.
- **One owner for Personal settings:** only `personal_settings.rs` reads or writes `personal_settings`, tests included; values are secrets in logs, and a secret never goes back to a client (ADR 0046).
- **Private modules.** The area's files are private modules, except `personal_settings`, which `events` names; neighbours import only what `../users.rs` re-exports, and items inside default to `pub(super)`.

### Neighbours

- Uses: `db`, `plugins` (`ExtensionPoints`, the Authenticator) and `forwarded` (`RequestSource`).
- Used by: `api` (sessions, `SignIn`, `SetupCode`, the Users parts), `connections` (`random_secret`), `events` (Personal settings for Notifier deliveries), `playback`, `requests`, startup in `lib.rs` and `main.rs`, and the tests of `automation`, `browse` and `library`.
