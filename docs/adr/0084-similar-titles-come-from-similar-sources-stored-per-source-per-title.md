# Similar titles come from Similar sources, stored per source per title

Similar titles came only from the title source that had a title (ADR 0081), so no other Plugin could add to them. Similar titles are now their own Extension Point, a Similar source served by `SimilarSourceService`, and title sources only describe titles. The TMDB Plugin's `recommendations` move to its Similar source answer, and `similar` leaves the title-source `Movie` and `Show`, with tag 16 reserved. A Similar source receives a title and its kind, never the User, and answers titles by External ID, each with a weight. The Kernel asks every enabled Similar source when it refreshes a title's Matches (ADR 0040) and stores each source's answer once per title (ADR 0081), so a title page never waits on a Plugin. The Kernel merges the stored answers when a page reads them, using Featured's rule (ADR 0072): each weight is clamped to between 0.0 and 1.0, a title answered by more than one source keeps its highest weight, and ties go to the source with the lower Plugin id. It keeps only the titles in the reading User's Libraries, matched by any shared External ID, and shows the first 20 as one row named "Similar titles".

## Considered Options

- **Merge across every title source's `similar`**: rejected. Only title sources could add to it, so a Plugin based on genres or Roles couldn't take part.
- **Ask for per-User answers when the page opens, with a timeout and a short cache**: deferred. It breaks "a page never waits on a Plugin" (ADR 0058), and a source that learns from the User reads their watch history, which needs its own decision on Plugin Permissions (ADR 0049). A later `user_similar` export can add this without changing the shared one, as `user_rows` sits beside `shared_rows`.
- **Every title for every User, ahead of time**: rejected. The cost grows with titles times Users.
- **Ask every source on every visit and stream the answers in, as search does (ADR 0080)**: rejected. Every visit would cost a call to each source, and the row would fill in late.
- **Merge at refresh and store only the result**: rejected. Disabling a Plugin couldn't drop its titles, a failed call couldn't keep that source's previous answer, and changing the merge rule would need a refresh of every title.
- **One row per source**: rejected. The row would show which Plugin each title came from, which ADR 0058 avoids.
- **TMDB keeps its recommendations from the title fetch to answer Similar without a second call**: rejected. It couples two exports inside the Plugin. Instead TMDB makes one more call per title refresh, and refreshes are rare.

## Consequences

- An enabled Similar source's answers appear at each title's next refresh. Nothing backfills the whole Library, since that would be one call per title.
- A source that fails keeps its previous stored answer and counts toward disabling the Plugin, like any failed call. A disabled Plugin's answers are dropped.
- Pages for titles not on the Server (`FoundMovie`, `FoundShow`) are already live, so they ask each Similar source live, with the same timeout as the title fetch. An empty answer or a timeout gives an empty row, not an error.
- There is no Admin factor per source. As in ADR 0072, enabling a Plugin is the Admin's trust in its weights.
- A local source based on genres and Roles needs a Kernel function that gives a title's Roles to Plugins. That is a separate issue.
