# Client lists page by keyset with shared page messages

A client list that can grow without bound (a Library's Movies or Shows, Requests, Unmatched Items, search results, Continue Watching) answers a page at a time. Its request embeds `nama.v1.PageRequest { size, token }` and its response `nama.v1.PageResponse { next_token, total_size }`. A missing `size` means 50, and more than 100 is lowered to 100. There is no "everything" size, because a Server on a 1 GiB machine (ADR 0039) mustn't build a whole Library in one answer. The token is an opaque keyset cursor: the last row's sort key and id, and a hash of the filter and sort it was made under. A token used with another filter or sort is refused with `REFUSAL_REASON_PAGE_TOKEN_INVALID`. Rows added or removed between pages don't shift later pages, as offsets would. `total_size` is an exact count, given on the first page only: SQLite has no estimate for a filtered query (`sqlite_stat1` counts whole tables), and counting again on each page is waste. Each list has its own typed filter message and a typed sort enum, one key, with a shared `SortDirection`, always tie-broken by id. The keyset runs in SQL (`WHERE (key, id) > (?, ?) ORDER BY key, id LIMIT n + 1`), so a page reads only its rows.

## Considered Options

- **A filter string and an `order_by` string (AIP-160, AIP-132):** rejected. It's a parser and its errors for a first-party client, with nothing the compiler checks.
- **Flat `page_size` / `page_token` fields on each request (AIP-158):** rejected. Shared messages give one place on the Server to clamp sizes and check tokens, and one web hook (connect-query takes the `page.token` path).
- **Offset paging:** rejected. It repeats or skips rows when a Scan adds or removes titles between pages.
- **A `page_size` of 0 for everything, as `list_titles` keeps for Plugins (ADR 0048):** rejected for clients. The web app is the only client and moves with the Server, so there's no old client to keep working.

## Consequences

- Titles sort by a stored sort title: Unicode lowercase folded in Rust, with a leading English "The", "A" or "An" dropped. SQLite here has no ICU collation, and `NOCASE` folds ASCII only.
- Config lists that stay small (Libraries, Users, Devices, Plugins, Qualities), one Season's Episodes, and searches answered by an outside source's own page aren't paged.
- The web app reads paged lists through one hook over `useInfiniteQuery`, holding at most 10 pages, so an event that invalidates a list refetches at most 1,000 rows.
