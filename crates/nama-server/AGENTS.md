## nama-server

### Data

- **One database writer.** Every write goes through `Database::write` in `src/db.rs`, on the one writable SQLite connection. Long jobs such as a scan write in small batches.
- **One owner for a Library's contents.** Only `src/contents.rs` reads or writes `library_files`, `library_items`, `item_matches` and `match_pins`, tests included. Everything else goes through `Contents`.
- **Query data.** After changing a `sqlx::query!` or anything in `migrations/`, regenerate `.sqlx/` at the workspace root and commit it (commands in `README.md`).
- **Whole-Library reads.** Browse reads a Library's whole contents even to find one title. Each such read is marked `// ponytail: whole-Library read`; query by id there once Libraries get big.

### Security

- **Secrets stay out of storage and logs.** The database holds the Authenticator's credential string and the SHA-256 hash of each session token; only `src/users.rs` derives them. Types holding a token or Sign-in Code skip `Debug`. The Setup Code is the one secret logged, on purpose. Plugin settings are the one secret stored as written, since a Plugin needs a key such as TMDB's itself to send it: no setting value is ever logged or sent to a client (`SettingValue`'s `Debug` hides it).
- **Clients name ids.** Media is served by Version id (`Contents::version_path`) and artwork only for paths some Match stores (`Contents::stores_image`). Where a Library lives on disk is shown to Admins only.
- **New RPCs start closed.** `Access::of` in `src/api.rs` requires a session for any RPC outside `Access::PUBLIC` and `Access::SELF_SERVICE`; opening one is an explicit entry there.
- **Sensitive RPCs need Recent Authentication.** An RPC that changes who can sign in as whom (a password, a Sign-in Method, a Role, deleting a User, approving a Sign-in Code) goes in `NEEDS_RECENT_AUTHENTICATION` in `src/api.rs` (ADR 0014). The access layer checks it before the handler runs; handlers don't check it themselves.
- **Guessable requests count as failures until proven otherwise.** A request that checks a password or the Setup Code calls `SignInLimits::admit` first and un-counts the attempt with `succeeded` only once the secret checked out; any other failure, including one before the check, stays counted. Sign-in Codes are the exception: `SignInCodeLimits` counts only an unknown or expired code.
- **Forwarded headers from trusted proxies only.** Read where a request came from (client IP, HTTPS) from the `RequestSource` that `src/forwarded.rs` sets. It believes `X-Forwarded-*` only from a peer in `Config::trusted_proxies` (ADR 0018), since any client can set them.
- **Causes stay server-side.** A database or server-side error is logged; the client gets a generic Connect error.
- **Refusals are typed.** Every error a client can cause carries a `nama.v1.Refusal` detail (ADR 0019). Build it with `refuse` in `src/api.rs`, which sets the Connect code from the `RefusalReason` and fills `debug`; never build a client-caused `ConnectError` directly. A refusal about one request field names it in `field`, and values its message mentions go in `params` (AIP-193: lowerCamelCase keys, units in the key). Server-side failures carry none.
- **Plugin code runs contained.** Every call into a Plugin, its `Drop` included, goes through `contain_sync` (or its async twin) in `src/plugins.rs`, outside the registry lock.
