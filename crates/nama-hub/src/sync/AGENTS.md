## Sync

Sync (ADR 0008, ADR 0092): each Account's merged Watch State and Subtitle and Audio Preferences, which its Linked Users' Servers push their changes into and pull from, and its merged Watchlist (ADR 0056), which they sync whole.

- **One owner.** Only this area reads or writes `watch_states`, `track_preferences` and `watchlists`, tests included.
- **Keyed by External ID** (ADR 0013): a title known by several has a row under each, so a Server finds it under any it knows. An Unmatched Item has none, so it never reaches the Hub.
- **The change made last wins**, per Account, title and id, or per Account and Preference, by the time the Server made it, not when it arrived. Marking a title unplayed is a change like any other: unplayed at 0 of 0.
- **A Preference is a description** (ADR 0035): a Track's language, forced, SDH and title, never which Track it was, since each Server matches it against its own. Home layouts, Permissions, Library access and Plugin settings belong to each Server and never come here (ADR 0092).
- **Scoped by Account** (ADR 0097): a Linked Server's change reaches only the Account its User is Linked to, and one of a User who isn't Linked is dropped. A pull reads only that Account's, and nothing for a User who isn't Linked.
- **A Watchlist keeps removals.** A title taken off stays, `removed`, so a Server that still has it takes the removal; the change made last wins as above, and of two made at once the removal, on the Hub and the Server alike, so they settle on the same one. Its titles are kept by the External ID a Server added them under, whether or not any Server has them.
- **Watchlists count across Accounts only as Viewing Events pool** (ADR 0093): the most watchlisted reads a title, by kind and id, only once `MIN_POOLED_ACCOUNTS` Accounts have it on, counts no Account that turned pooling off, and answers External IDs and weights alone.
- **A pull hands each id's row apart.** The Hub doesn't know which ids name one title; the Server does for the titles it has, and merges them.

### Neighbours

- Uses: `links`, which `api` hands to each Linked Server call (the Accounts a Linked Server's Users are Linked to), `accounts` (`AccountId`) and `viewing` (who pools, and `MIN_POOLED_ACCOUNTS`).
- Used by: `api`, which checks a push against `MAX_CHANGES`, `MAX_IDS`, `MAX_ID_LENGTH`, `MAX_LANGUAGE_LENGTH`, `MAX_TITLE_LENGTH` and `MAX_CHANGED_AT_MS`, and a Watchlist against `MAX_WATCHLIST_ENTRIES`, `MAX_ID_LENGTH` and `MAX_CHANGED_AT_MS`, and converts them.
