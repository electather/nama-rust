# The Kernel owns Users, Devices and sessions; Plugins only check credentials

Users, Devices and their sessions are Kernel state (ADR 0006 lists auth sessions in the Kernel). How a person proves who they are is a Provider: the `local-auth` Built-in Plugin checks passwords, and in M5 the Hub becomes another one. A Provider answers only "these credentials belong to this User". It never issues or checks sessions, so every client and every route authenticates the same way, whichever Plugin signed the User in. Plugins have no storage of their own, so the Kernel keeps one opaque credential string per User per Plugin (for `local-auth`, an argon2id hash) that only that Plugin can produce and check.

A Device's session is an opaque random token, stored hashed and deleted to revoke the Device. Browsers send it as an HttpOnly cookie, because a `<video>` range request can't carry an `Authorization` header, and native clients send it as a bearer token. Both resolve to the same Device.

## Considered Options

- **JWTs for Server sessions**: rejected. A Device must be revocable at once, which a self-contained token can't do without a denylist. Hub-signed JWTs (ADR 0011) are only presented to a Server to start a session, never used as one.
- **Signed media URLs**: rejected. A second credential just for `/media/...`, and one that leaks through logs and shared links.
