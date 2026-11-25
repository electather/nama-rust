## Web Push

Pushing each inbox notification to every browser its User turned Web Push on in (RFC 8030), encrypted for that browser (RFC 8291) and signed with the Server's VAPID key (RFC 8292).

- **One owner for push subscriptions.** Only `subscriptions.rs` reads or writes `push_subscriptions`, tests included, and pushes to them what `../events.rs` stores in an inbox.
- **No third party.** The Server encrypts and signs each push itself, on `ring`, and posts it straight to the browser's push service. Subscribe takes only https endpoints, so a client can't make the Server post to whatever it names.
- **A push never holds anyone up.** Each event's pushes run in a task of their own, every subscription at once, each bounded by `PUSH_TIMEOUT`. A 404 or 410 deletes that subscription.

### Neighbours

- Uses: `db`, `diagnostics` (the TLS provider).
- Used by: `events` (pushing what it stored in inboxes), `server_settings` (`VapidKey`, which it makes and keeps), the `api` parts (`WebPushService`).
