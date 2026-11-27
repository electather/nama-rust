## Web Push

Pushing each inbox notification to every browser its User turned Web Push on in (RFC 8030), encrypted for that browser (RFC 8291) and signed with the Server's VAPID key (RFC 8292).

- **One owner for push subscriptions.** Only `subscriptions.rs` reads or writes `push_subscriptions`, tests included, and pushes to them what `../events.rs` stores in an inbox.
- **A subscription goes with its Device's User.** `subscribe` records the caller's Device, and the foreign key to `device_users` deletes the subscription however its User is signed out of that Device, so the browser's push subscription follows whoever is signed in to it (#2314). The web app's half is in `web/src/AGENTS.md`. A row from before #2314 has no Device and stays.
- **No third party.** The Server encrypts and signs each push itself, on `ring`, and posts it straight to the browser's push service. Subscribe takes only https endpoints, and a push follows no redirect, so a client can't make the Server post over plain http or wherever a redirect points.
- **A push never holds anyone up.** Each event's pushes run in a task of their own, every subscription at once, each bounded by `PUSH_TIMEOUT`. A 404 or 410 deletes that subscription.

### Neighbours

- Uses: `db`, `diagnostics` (the TLS provider), and `users` (`testing::insert_user` and `testing::insert_device`) in tests.
- Used by: `events` (pushing what it stored in inboxes), `users` (its tests), `server_settings` (`VapidKey`, which it makes and keeps), the `api` parts (`WebPushService`).
