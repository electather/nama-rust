# The Hub records Viewing Events and pools them across Accounts only as counts

Watch State keeps only the latest progress (ADR 0008), so recommendations get no history from it. A Linked Server therefore also sends the Hub a Viewing Event for each play by a Linked User, keyed by External ID. The event records start, stop point, whether the play finished, time of day and Device kind. Unmatched Items never leave their Server, and a Server records no Viewing Events of its own: unlinked Users have Watch State only, and a User's history starts when they Link.

Each Account's Viewing Events are stored under that Account and feed its own recommendations. Across Accounts, the Hub keeps only counts of how many Accounts watched each pair of titles, and each title each week, for charts such as what's trending. A count is used once at least *k* distinct Accounts are in it (k = 20 to start). No pooled record is kept per person, pseudonymous or otherwise. Pooling is on by default for Linked Accounts, shown at Linking, and each Account can turn it off.

## Considered Options

- **Pooled histories under a pseudonymous id**: rejected. A viewing history fingerprints a person, so a random id doesn't make it anonymous.
- **Differential privacy on the counts**: deferred. Counts with a k threshold expose nothing about any one Account to Nama's own recommender. Noise is worth its tuning only if the data is ever shared outside the project.
- **Pooling off by default** (ADR 0054's precedent for Diagnostics): rejected. Counts carry no person, and pair counts from few opt-ins are too sparse to recommend from. Diagnostics describe a household's Server; these counts describe nobody.
- **The Server's Admin decides pooling for its Users**: rejected. An Account belongs to a person, not to a Server.

## Consequences

- Turning pooling off stops new counts. Past counts can't be taken back out because they can't be traced to the Account, and the opt-out says so.
- Viewing Events are kept as long as their Account exists. An Account can see them, delete any one of them, which recomputes its taste vector, and export them all.
- Deleting an Account deletes its Viewing Events, Watch State and Watchlist from the Hub. Pooled counts stay.
- Charts of Watchlists, such as the most watchlisted, follow the same rules: the Hub counts the Watchlists it syncs (ADR 0056) as it reads them, uses a title once *k* Accounts have it on theirs, and leaves out an Account that turned pooling off, so turning it off takes its Watchlist back out.
