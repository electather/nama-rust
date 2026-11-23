## Sync

Sync (ADR 0008): each Account's merged Watch State, which its Linked Users' Servers push their changes into and pull from, and its merged Watchlist (ADR 0056), which they sync whole.

- **One owner.** Only this area reads or writes `watch_states` and `watchlists`, tests included.
- **Keyed by External ID** (ADR 0013): a title known by several has a row under each, so a Server finds it under any it knows. An Unmatched Item has none, so it never reaches the Hub.
- **The change made last wins**, per Account, title and id, by the time the Server made it, not when it arrived. Marking a title unplayed is a change like any other: unplayed at 0 of 0.
- **Scoped by Account** (ADR 0097): a Linked Server's change reaches only the Account its User is Linked to, and one of a User who isn't Linked is dropped. A pull reads only that Account's, and nothing for a User who isn't Linked.
- **A Watchlist keeps removals.** A title taken off stays, `removed`, so a Server that still has it takes the removal; the change made last wins as above, and of two made at once the one kept first. Its titles are kept by the External ID a Server added them under, whether or not any Server has them.
- **A pull hands each id's row apart.** The Hub doesn't know which ids name one title; the Server does for the titles it has, and merges them.

### Neighbours

- Uses: `links` (the Account a Linked Server's User is Linked to) and `accounts` (`AccountId`).
- Used by: `api`, which checks a push against `MAX_CHANGES`, `MAX_IDS`, `MAX_ID_LENGTH` and `MAX_CHANGED_AT_MS`, and a Watchlist against `MAX_WATCHLIST_ENTRIES`, and converts them.
