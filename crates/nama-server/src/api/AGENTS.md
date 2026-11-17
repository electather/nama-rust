## API

The Connect handlers: one part per service in this folder, and `../api.rs` with what they share.

- **Handlers stay thin.** A handler converts proto, takes the caller, calls the one owner of what it reads or changes and maps that owner's errors to Refusals. It holds no SQL and no rule another owner keeps.
- **Top row.** `api` sits in the top row of `ROWS` in `../../tests/structure.rs`, so handlers may call every area and single file below it, and nothing below calls `api`.
- **One part per domain.** An RPC's handler, its proto conversions and the mapping of its module's errors to Refusals go in its domain's part here, such as `libraries.rs` or `sign_in.rs`. `../api.rs` holds only the router, the access layer (`ACCESS`, the check that reads it, `NEEDS_RECENT_AUTHENTICATION`), `refuse` with the reason table, and helpers more than one part uses. A conversion two parts use lives with the part that owns its type.
- **New RPCs start closed.** `ACCESS` in `../api.rs` declares, once per RPC, who may call it: anyone, any signed-in User, or a User whose Permissions pass a check for one Permission (`Permissions::allow` in `../users.rs`: `ADMIN` passes every check) (ADR 0020). An RPC missing from it is refused to everyone, session or not, with the router's own 404 for an unknown path (no Refusal), and a test fails until it is listed. Paths that aren't RPCs are declared by prefix in `PREFIXES`: media under `/media/` needs a session, and uploading or removing a Custom Image under `/custom-images/` needs `MANAGE_LIBRARIES`, or under `/custom-images/plugin/`, `MANAGE_SETTINGS`. A self-service RPC (your own Devices or password) is signed-in and its handler checks ownership.
- **Sensitive RPCs need Recent Authentication.** An RPC that changes who can sign in as whom (a password, a Sign-in Method, a User's Permissions or the default ones, deleting a User, approving a Sign-in Code), or that deletes media, goes in `NEEDS_RECENT_AUTHENTICATION` in `../api.rs` (ADR 0014). The access layer checks it before the handler runs; handlers don't check it themselves.
- **Causes stay server-side.** A database or server-side error is logged; the client gets a generic Connect error.
- **Refusals are typed.** Every error a client can cause carries a `nama.v1.Refusal` detail (ADR 0019). Build it with `refuse` in `../api.rs`, which sets the Connect code from the `RefusalReason` and fills `debug`; never build a client-caused `ConnectError` directly. The one exception is the router's 404 for a path no `ACCESS` entry lists (see "New RPCs start closed"). A refusal about one request field names it in `field`, and values its message mentions go in `params` (AIP-193: lowerCamelCase keys, units in the key). Server-side failures carry none.

### Neighbours

- Uses: every area and single file below the top row: `browse`, `scanner`, `plugin_settings`, `requests`, `playback`, `matching`, `library`, `events`, `connections`, `users`, `plugins`, `images`, `server_settings`, `ffmpeg`, `db`, `config` and `diagnostics` (the span each RPC runs in).
- Used by: startup in `lib.rs` (`router`).
