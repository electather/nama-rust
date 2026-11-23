## Recommendations

What the Hub recommends to a Linked User for their Featured and Home rows (ADR 0072): the titles nearest the taste vector of the Account they are Linked to, built from the titles its Viewing Events finished (ADR 0093, 0095).

- **No table of its own.** The taste vector is built on each ask from `viewing`'s events and `titles`' vectors, so deleting a Viewing Event or a new training run changes the next answer.
- **Answers are External IDs with weights**, never names or anything from TMDB or TVDB (ADR 0094). The Server stores them with a time to live; nothing here does.
- **Finished titles aren't recommended.**
- **Scoped by Account** (ADR 0097): a Linked Server gets only its own Users' recommendations, and none for a User who isn't Linked.

### Neighbours

- Uses: `links` (the Account a Linked Server's User is Linked to), `viewing` (the Movies the Account finished) and `titles` (`recommended`, `ExternalId`, `TitleKind`, `RecommendedTitle`).
- Used by: `api`, which converts what it answers.
