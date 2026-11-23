## Viewing

Viewing Events (GLOSSARY.md): each play by a Linked User, recorded by their Linked Server under the Account they are Linked to, and listed and deleted by that Account, and the pooled counts of how many Accounts finished each Movie and pair of Movies, and each Movie each week (ADR 0093).

- **One owner.** Only this area reads or writes `viewing_events`, `pooled_counts`, `weekly_counts` and `pooling_settings`, tests included.
- **Keyed by External ID**, as the Server keeps Watch State (ADR 0013), and an Episode's event by its Show's too (`show_external_id`), since `titles` hold Shows, never Episodes. The Hub never stores a title's name: that is the title source's content (ADR 0094).
- **Only Linked Users' plays.** An event for a User who isn't Linked is left out, not refused, so a Server's queue empties after an Unlink.
- **Sent again, recorded once.** A Server names each event by a key, unique per Account, so a retry after a lost answer adds nothing.
- **Kept as long as the Account.** No expiry: rows go with the Account, by foreign key, or one at a time when it deletes them.
- **Pooled counts name nobody.** `pooled_counts` and `weekly_counts` hold External IDs, a week in `weekly_counts`, and a number of Accounts, never anything that points to one, even pseudonymously. A count is read only once `MIN_POOLED_ACCOUNTS` (20) Accounts are in it.
- **Each Account counts once.** Its first finished play of a Movie, while it pools, counts it for that Movie and for each pair with a Movie it was counted for before; `viewing_events.pooled` marks that play. Deleting it forgets the mark, so finishing the Movie again counts the Account again (a `ponytail:` ceiling in `viewing.rs`). Its first finished play of a Movie in a week counts it in `weekly_counts` for that week, which `viewing_events.pooled_week` marks; deleting that play takes it back out, since every Linked Server reads that count. Episodes aren't pooled yet, though their events name their Show.
- **Pooling is on by default**, and an Account turns it off in `pooling_settings`. Off stops new counts; past ones stay, as nothing traces them back.
- **Scoped by Account** (ADR 0097): an Account lists and deletes only its own; a Linked Server records only under its own Users' Accounts.

### Neighbours

- Uses: `links` (the Account a Linked Server's User is Linked to), `accounts` (`AccountId`) and `titles` (`ExternalId`, `PooledCount`).
- Used by: `api`, which converts what `viewing.rs` defines, `lib`, which trains title vectors on `pooled_counts`, `recommendations` (an Account's finished Movies), and `sync`, which counts Watchlists by the same pooling rules.
