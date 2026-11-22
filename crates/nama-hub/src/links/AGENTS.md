## Links

Links (GLOSSARY.md): registering a Server through an OAuth device flow (ADR 0011), its credential, its Server Addresses, Unlinking, and the tokens signed for Servers (ADR 0099); and Linking a Linked Server's Users to Accounts (`users.rs`).

- **One owner.** Only this area reads or writes `linked_servers`, `server_link_codes`, `user_links` and `user_link_codes`, tests included.
- **The Server polls; the Hub never calls it.** A Server starts with a device code it keeps and a code a person enters. Entering it makes the Linked Server; the Server's next poll collects its credential and deletes the codes.
- **Credentials are opaque** (ADR 0098): 32 random bytes, stored only as their SHA-256 hash, revoked by deleting the row. The token a Server gets with it is the only thing signed, so it can check which Account it is Linked to.
- **A User Links like a Server**, but their Server asks with its credential, so a User's code needs no device code: the Server asks `GetUserLink` until it is entered. A User Links to one Account, and an Account to Users on many Servers. Unlinking a Server Unlinks its Users.
- **Server Addresses come from the Server**, which reports them as they change. The Hub keeps only the last ones, and checks each is an `http` or `https` URL, since its pages link to them. Clients reach Servers there themselves: the Hub never proxies a call or media (ADR 0094).
- **Only a proven email Links** (ADR 0100), Servers and Users alike, and only from a session with Recent Authentication.
- **Scoped by Account** (ADR 0097): an Account lists only the Linked Servers it owns or has a User on, lists and Unlinks only its own Linked Users, and a Linked Server Unlinks only itself and its own Users.

### Neighbours

- Uses: `accounts` (an `AccountSession`'s proven email and Recent Authentication, an Account's email) and `signing` (the token).
- Used by: `api`, whose auth layer turns a bearer credential into a `LinkedServer`, and which converts what `links.rs` re-exports, and `diagnostics` (`LinkedServerId`, each Server's volume).
- Used by: `sync`, for the Account a Linked Server's User is Linked to.
