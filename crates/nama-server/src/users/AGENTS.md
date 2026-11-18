## Users

Users, Devices and their sessions (`../users.rs`), what their names may hold and the key a User's, and a Library's, is unique under (`names.rs`), the Permissions a User holds (`permissions.rs`), setting and checking passwords through the Authenticator (`passwords.rs`), changing other Users' Permissions, the default ones, and deleting Users (`managing.rs`), Invites (`invites.rs`), signing in (`sign_in.rs`, with Sign-in Codes in `sign_in_codes.rs`, the Setup Code and random codes in `codes.rs`, the sign-in limits in `rate_limits.rs` and Device names in `device_names.rs`), the session layer over HTTP (`sessions.rs`), Personal settings, the kinds of Request notification each User turned off (`notification_preferences.rs`), and whose Requests and playing a User sees (`seeing.rs`: `User::whose_requests`, `User::whose_playing`, `may_see`, `is_requester`).

- **Guessable requests go through `SignIn` (`sign_in.rs`).** It lets an attempt through before reading a User or a secret, and un-counts it the moment the secret checks out; a failure after that, an input check before it, or a refusal any client could predict, such as Setup on a set-up Server, is no guess. Sign-in Codes count only an unknown or expired code.
- **One owner for Personal settings:** only `personal_settings.rs` reads or writes `personal_settings`, tests included; values are secrets in logs, and a secret never goes back to a client (ADR 0046).
- **Private modules.** The area's files are private modules; neighbours import only what `../users.rs` re-exports, and items inside default to `pub(super)`.

### Neighbours

- Uses: `db`, `plugins` (`ExtensionPoints`, the Authenticator) and `forwarded` (`RequestSource`).
- Used by: `api` (sessions, `SignIn`, `SetupCode`, the Users parts, Invites), `connections` (`random_secret`), `events` (Personal settings and turned-off notifications for Notifier deliveries, `whose_requests`, `whose_playing` and `may_see`), `playback`, `requests` (`whose_requests`, `may_see` and `is_requester`), `library` (`name_key`), startup in `lib.rs` and `main.rs`, and the tests of `automation`, `browse` and `library`.
