# A title's Roles, Trailers and Similar titles are stored once per title

A Match copies its Show's metadata, such as its genres and overview, onto every Episode's Match. That works for a few fields, but a Show's ~50 Roles, its Trailers and its Similar titles copied onto each of 1,000 Episodes would be mostly duplicates. So these lists, and a title's Score, are stored once per title, keyed by its External ID. They are fetched in the same call as the rest of the title and refreshed whenever its Matches are (ADR 0040). This is the first per-title store, and it sits beside the per-Match copies on purpose. A Person's headshot is stored once per Person and shared by all their Roles.

## Considered Options

- **Copy them onto each Match, like genres**: rejected. Storage grows with the Episode count, and refreshing a Show rewrites every Episode.
- **Fetch them live when a page asks**: rejected for the reasons ADR 0040 gives. The exception is pages for titles not on the Server (`FoundMovie`, `FoundShow`), which are live already.

## Consequences

- Only Roles, Trailers, Similar titles and Score live here. Moving the per-Match copies of Show fields into it would be a separate change.
- A Similar title counts as in a User's Libraries when it shares any External ID with a Matched title. One that shares none is dropped, and nothing looks it up in another Namespace.
