## nama-server

### Data

- **One database writer.** Every write goes through `Database::write` in `src/db.rs`, on the one writable SQLite connection. Long jobs such as a scan write in small batches.
- **One owner for a Library's contents.** Only `src/contents.rs` reads or writes `library_files`, `library_items`, `item_matches` and `match_pins`, tests included. Everything else goes through `Contents`.
- **One owner for Tracks.** Only `src/tracks.rs` reads or writes `version_probes`, `tracks` and `probe_failures`, tests included. Browse attaches them to the Versions `Contents` reads.
- **One owner for probing.** Only `src/probing.rs` runs ffprobe on stored files and decides what a probe failure means for trying it again (`ProbeFailure`); Tracks stores it.
- **One owner for Quality rules.** Only `src/qualities.rs` reads or writes `qualities` and `default_qualities`, tests included, and decides which Quality a Request, Show or Followed Show takes (ADR 0025). Requests and handlers ask it.
- **One owner for Server settings.** Only `src/server_settings.rs` reads or writes `server_settings`, tests included.
- **One path for Request changes.** Every change to a Request or Followed Show goes through `Requests::apply`, which alone publishes their events and hands off (ADR 0029). A Library's write making them Available is the exception (ADR 0012).
- **Requester rules once.** What a Request and a Followed Show share (Requesters, the guarded Pending decision, who may see one) is written once in `src/requests/asked.rs`; each keeps only what differs.
- **One owner for Plugin changes.** Outside tests, only `src/plugin_settings.rs` enables or disables a Plugin or saves its settings, under its one lock and in ADR 0028's order. Handlers and tasks ask `PluginSettings`; they don't call `Registry::enable_by_id` or `Registry::disable`.
- **Query data.** After changing a `sqlx::query!` or anything in `migrations/`, regenerate `.sqlx/` at the workspace root and commit it (commands in `README.md`).
- **Name parser version.** A change to how names are parsed or how Matching decides that could turn an Unmatched Item into a Match bumps `PARSER_VERSION` in `src/scanner/names.rs`, so the next scan parses stored files again and retries their automatic Unmatched decisions (ADR 0023).
- **Probe version.** A change to what probing reads from a file, or how, bumps `PROBE_VERSION` in `src/tracks.rs`, so the next pass after a scan probes every file again (ADR 0023's pattern).
- **Probe failure version.** A change that may let ffprobe read files it lastingly failed on before, without changing what probing reads from a file that it could read, bumps `PROBE_FAILURE_VERSION` in `src/tracks.rs`, so the next pass tries again only those files.
- **WebVTT version.** A change to the WebVTT a subtitle source makes bumps `WEBVTT_VERSION` in `src/playback/subtitles.rs`, so what the cache kept from before is made again.
- **Whole-Library reads.** Browse reads a Library's whole contents even to find one title. Each such read is marked `// ponytail: whole-Library read`; query by id there once Libraries get big.

### API

- **One part per domain.** An RPC's handler, its proto conversions and the mapping of its module's errors to Refusals go in its domain's part under `src/api/`, such as `libraries.rs` or `sign_in.rs`. `src/api.rs` holds only the router, the access layer (`ACCESS`, the check that reads it, `NEEDS_RECENT_AUTHENTICATION`), `refuse` with the reason table, and helpers more than one part uses. A conversion two parts use lives with the part that owns its type.

### Security

- **Secrets stay out of storage and logs.** The database holds the Authenticator's credential string and the SHA-256 hash of each session token; only `src/users.rs` derives them. Types holding a token or Sign-in Code skip `Debug`. The Setup Code is the one secret logged, on purpose. Plugin settings are the one secret stored as written, since a Plugin must send a key such as TMDB's as it is: no setting value is ever logged (the `Debug` of `SettingValue` and `PluginTables` hides them), and a secret's value, stored or default, is never sent to a client (`setting_to_proto` in `src/api/plugins.rs`).
- **Clients name ids.** Media is served by Version id (`Contents::version_file`) and artwork only for paths some Match stores (`Contents::stores_image`) or a Movie or Show search or lookup handed out (`Artwork::offer`). Where a Library lives on disk is shown only to Users with `MANAGE_LIBRARIES`.
- **New RPCs start closed.** `ACCESS` in `src/api.rs` declares, once per RPC, who may call it: anyone, any signed-in User, or a User whose Permissions pass a check for one Permission (`Permissions::allow` in `src/users.rs`: `ADMIN` passes every check) (ADR 0020). An RPC missing from it is refused to everyone, session or not, with the router's own 404 for an unknown path (no Refusal), and a test fails until it is listed. Media under `/media/` needs a session (`MEDIA`). A self-service RPC (your own Devices or password) is signed-in and its handler checks ownership.
- **Sensitive RPCs need Recent Authentication.** An RPC that changes who can sign in as whom (a password, a Sign-in Method, a User's Permissions or the default ones, deleting a User, approving a Sign-in Code) goes in `NEEDS_RECENT_AUTHENTICATION` in `src/api.rs` (ADR 0014). The access layer checks it before the handler runs; handlers don't check it themselves.
- **Guessable requests count as failures until proven otherwise.** A request that checks a password or the Setup Code lets the attempt through with `SignInLimits::admit` before it reads a User or a secret, and un-counts it with `succeeded` the moment the secret checks out; an input check before `admit` (the delivery, the Device name) is no guess, and any other failure stays counted. Signing in with a name and password does this in `SignIn` (`src/sign_in.rs`). Sign-in Codes are the exception: `SignInCodeLimits` counts only an unknown or expired code.
- **Forwarded headers from trusted proxies only.** Read where a request came from (client IP, HTTPS) from the `RequestSource` that `src/forwarded.rs` sets. It believes `X-Forwarded-*` only from a peer in `Config::trusted_proxies` (ADR 0018), since any client can set them.
- **Causes stay server-side.** A database or server-side error is logged; the client gets a generic Connect error.
- **Refusals are typed.** Every error a client can cause carries a `nama.v1.Refusal` detail (ADR 0019). Build it with `refuse` in `src/api.rs`, which sets the Connect code from the `RefusalReason` and fills `debug`; never build a client-caused `ConnectError` directly. The one exception is the router's 404 for a path no `ACCESS` entry lists (see "New RPCs start closed"). A refusal about one request field names it in `field`, and values its message mentions go in `params` (AIP-193: lowerCamelCase keys, units in the key). Server-side failures carry none.
- **Plugin code runs contained.** Every call into a Plugin, its `Drop` included, goes through `contain_sync` (or its async twin) in `src/plugins.rs`, outside the registry lock.

### Playback

- **What a plan converts is worked out once.** Before the steps run, `candidates` in `src/playback.rs` works out what a Remux and a Transcode of the Version would convert, and the plan keeps the one it narrowed to in `Playing`. Its reasons, Tracks, events and ffmpeg arguments read that value; none asks the planning facts again what it converts.

### Tests

- **One fixture.** Server tests build on `TestServer` in `tests/support`, started through `TestServer::builder()`. Their setup goes through the API, as a client's would; a shared helper never writes the database. A test file adds a fixture of its own only for what that file alone needs, and a shortcut one test needs stays in that test's file.
