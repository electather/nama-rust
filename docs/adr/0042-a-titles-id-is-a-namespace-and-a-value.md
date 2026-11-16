# A title's id is a namespace and a value

A title's id at a metadata source, its External ID, is a pair: a namespace and a value. A namespace is one or more lowercase ASCII letters and digits. A value is a non-empty string whose form the namespace's owner defines. The Kernel never interprets a value: it compares, stores and passes it on, and a Plugin that needs a number parses it itself. `tmdb`, `imdb` and `tvdb` are well-known namespaces whose values keep today's forms (`603`, `tt0078748`, `81189`); a Plugin may bring any other. Movies and Shows keep separate id spaces, so a kind still travels beside an id. A title may carry several ids, at most one per namespace, and any one of them identifies it. Where one must be picked, such as grouping a Library's Items into titles or keying a Request, the Kernel prefers `tmdb`, then `imdb`, then `tvdb`, then any other in namespace order, until per-Library source order exists. There is no surrogate title id: Watch State, track preferences, Custom Images, Requests, Followed Shows and Pinned Matches key on an External ID, as Watch State already does (ADR 0008). The text form is `namespace-value`, split at the first `-`, since a namespace has no `-` and a value may; ID Hints, Watch State keys, Custom Image paths and web routes already use it, and it is kept because it is simple, not for compatibility. Where it is one segment of a path, as in a web route or a Custom Image path, the value is percent-encoded. Ids order by namespace, then by value shortest first and then as text, so values that are numbers sort as numbers without being read as numbers. Fixed TMDB, IMDb and TVDB fields made every new metadata source a change to the schema, the client API and the Plugin contract; with open pairs a Plugin brings its own namespace and nothing else changes.

## Storage

- Where a row carries several ids of one title, a child table of (owner, namespace, value) holds one row per namespace: `item_match_ids` for a Match (the Item's own title and, for an Episode, its Show), and `request_title_ids` for a Request's title.
- Where a row keys on one id, one column holds its text form: `requests.title_id`, `followed_shows.show_id` and `match_pins.pinned_id`, as `watch_states.title_id`, `track_preferences.title_id` and Custom Image paths already do.
- `requests.failure_needs` holds the namespaces a Failed Request lacked, as one comma-separated text column (a namespace has no comma), because it is only shown, never queried by.
- The uniqueness rules are unchanged, over the new shape: one open Request per Movie or Season (`title_id`, season), one open Followed Show per Show, and one pin per name.
- An existing Server keeps its Matches (their ids are copied into `item_match_ids`), Pinned Matches, Watch State, track preferences and Custom Images.
- An existing Server loses every Request with its Requesters, and every Followed Show with its Requesters. Those tables are recreated empty, because carrying their rows through SQLite's foreign-key rules would cost more than the owner wants spent. Users ask again.

## Contracts

- The client API's `ExternalId` becomes a message of namespace and value, and the Request, Followed Show and look-up messages carry it in place of bare TMDB, IMDb and TVDB fields. The Server refuses a malformed namespace or an empty value with `invalid_argument`. Routes keep their shape (`/movies/tmdb-603`); old clients are not kept working.
- The Plugin contract carries a title's ids as a list of namespace and value pairs in a message of its own, importing nothing from the client API (ADR 0041). A Fulfiller names the namespaces it can find a title by, and any one of them is enough. The Kernel does not hand it a Request whose title has none of them, and the Request shows which id is missing.
- Nothing here stays backward compatible where that would limit the design. Dropped: the fixed `tmdb`, `imdb` and `tvdb` oneof and fields in both contracts, the six id columns of `item_matches`, the three of `match_pins`, `requests.tmdb_id`, `show_imdb_id` and `show_tvdb_id`, `followed_shows.tmdb_id`, and the stored Requests and Followed Shows.

## Considered Options

- **A surrogate title id with a mapping table**: rejected. Every table keyed on a title, and Watch State sync, would need the mapping, and ids from two sources for one title can't be merged safely without a source of truth.
- **Keep fixed columns and add one per source**: rejected. Every new source would be a schema, client API and Plugin contract change.
- **One text column holding all of a title's ids**: rejected. A title couldn't be looked up by any one of its ids.

## Consequences

- A Library's Match makes a Request Available when the two share any id, whichever id each prefers: a Request kept under `tvdb` becomes Available through a Match whose preferred id is `tmdb`, if the Match carries the same `tvdb` id. A Followed Show's scan finds it the same way.
- ID Hints in file and folder names accept any namespace some enabled title source keeps or looks titles up by (ADR 0044).
- Title metadata comes from each Library's title sources, not TMDB alone (ADR 0044).
- Watch State is written under a title's preferred id, which follows each Library's title source order, so reordering a Library or keeping a title in two Libraries can leave one User's Watch State of it under two ids. The Automation's reads find it under any of the title's ids, keeping the most recent per User (ADR 0048). Browse's reads and writes (a title's Watch State, reporting and marking progress, and Continue Watching) still key on the preferred id alone; finding it under any id there is deferred.
