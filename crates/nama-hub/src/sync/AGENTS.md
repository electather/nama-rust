## Sync

Sync (ADR 0008): each Account's merged Watch State, which its Linked Users' Servers push their changes into.

- **One owner.** Only this area reads or writes `watch_states`, tests included.
- **Keyed by External ID** (ADR 0013): a title known by several has a row under each, so a Server finds it under any it knows. An Unmatched Item has none, so it never reaches the Hub.
- **The change made last wins**, per Account, title and id, by the time the Server made it, not when it arrived. Marking a title unplayed is a change like any other: unplayed at 0 of 0.
- **Scoped by Account** (ADR 0097): a Linked Server's change reaches only the Account its User is Linked to, and one of a User who isn't Linked is dropped.

### Neighbours

- Uses: `links` (the Account a Linked Server's User is Linked to) and `accounts` (`AccountId`).
- Used by: `api`, which checks a push against `MAX_CHANGES`, `MAX_IDS`, `MAX_ID_LENGTH` and `MAX_CHANGED_AT_MS` and converts it.
