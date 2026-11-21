# A Plugin's Home row answer holds up to 20 rows

ADR 0058 let each Home row answer list up to 3 rows. The Discover Plugin offers 18 rows every User sees, such as Trending this week, Top rated movies and Best Picture winners, and 9 rows for each User, such as Because you watched and Your Watchlist. One answer of 3 would split it into several Plugins, each with its own TMDB key. So an answer, `shared_rows` or `user_rows`, now lists up to 20 rows, and the Kernel still cuts what goes over. Everything else in ADR 0058 holds: rows are stored ahead of time, each holds up to 200 External IDs, and a refresh replaces a Plugin's rows for its scope in one transaction. Stored rows are bounded by Plugins, Users, 20 rows and 200 titles.

A Plugin offering more rows than that lets the Admin pick which it serves, in a several-choices setting (ADR 0088), and its settings check refuses a save picking more than 20, so the Kernel never cuts a row the Admin asked for. Users hide what they don't want through their Home layout (ADR 0077).

## Considered Options

- **Keep 3 rows per answer and split Discover into several Plugins**: rejected. Each would need its own TMDB key, since a Plugin reads only its own settings (ADR 0041), and the Admin would enable and set up the same service several times.
- **No limit**: rejected. A Plugin could store thousands of rows per User; the limit is what bounds stored rows.
- **Asking providers other than TMDB for charts**: Trakt adds Most anticipated, box office and Most watched, but only under personal, non-commercial terms and with a second key; Simkl requires its name in each row's heading and a link per title, which Nama doesn't draw; per-country Top 10s are paid or partner-only, except JustWatch's chart through MDBList, which belongs in the MDBList Plugin. Discover asks TMDB for charts and searches, and Wikidata, which needs no key, for film awards.

## Consequences

- Discover's award rows cover film awards only: Wikidata's TV awards are patchy, so Emmys wait for a better source.
- The Hub may later fetch charts and awards once for every Server, so a Server needs no TMDB key of its own; nothing is built for it yet.
- The Kernel keeps building Continue Watching and Recently added itself; new rows come from Plugins, and a Plugin builds a Watchlist row from `list_watchlist`.
