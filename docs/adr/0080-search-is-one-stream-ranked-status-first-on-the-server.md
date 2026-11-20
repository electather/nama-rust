# Search is one stream ranked status-first on the Server

One server-streaming RPC, `SearchService.Search`, answers every search: Movies, Shows and Episodes in the Libraries, and the titles a title source has that the caller may Request. It replaces `BrowseService.SearchLibraries` and `RequestService.SearchMovies` / `SearchShows`. The Server sends a batch per source as each answers, the Libraries first, then a `Done` that flags a source that failed or is missing, so a slow or absent title source never holds back what the Server has. Each result carries a status: Available (at least one Episode or the Movie in a Library), Requested (an open Request the caller can see, for the Movie or any Season, or a Followed Show), or Not on this Server. Results are ranked status first, so a title the caller can watch always sits above a better-matching one they can't. Inside a batch, the Server orders them by how well the title matches (Movies and Shows before Episodes, then exact title, starts-with, all words, year, bm25, shorter title), and inside a title source's batch by the source's own order after that. A client stable-sorts the rows it has by status, which is the whole merge. A later source, such as a Search source, adds a batch: its results are de-duplicated by External ID and ordered by the Admin's precedence of Plugins, never by blending scores.

## Considered Options

- **Two client calls, with `ShowHit` gaining `library_id` and `request`**: rejected. Every client would rank, de-duplicate and check permissions on its own, and get it differently.
- **Match quality first, status as a tie-break**: rejected. A search that puts a title the User can't watch above one they can sends them to a Request they don't need.
- **A `score` on each result and per-source weights**: rejected. bm25, a title source's order and a cosine similarity aren't on one scale, so blending them is a knob nobody can tune; status, then each source's own order, is enough.

## Consequences

- A title-source result a Library holds that the Library search missed, such as under its original title, still appears, as Available, rather than being dropped.
- Episodes come only from the Libraries, since an Episode can't be Requested.
- Typo tolerance and indexing original titles are left to the Library batch; they change neither the RPC nor clients.
