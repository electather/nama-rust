# A Library asks its title sources in order

Movies and Shows are found and fetched through one Extension Point, the title source, `nama.plugin.v1.TitleSourceService`; the Built-in `TitleSource` trait mirrors it and speaks its messages, so a Built-in and an Installed source answer alike. It replaces the MetadataProvider name, and every method is optional (search, Movie, Show, Season, Episode, image, and later the artwork list): a source without one isn't asked for it. A source keeps its titles under one namespace (`Manifest.title_namespace`, `tvdb` for the TVDB Plugin). It may also declare the other namespaces it looks titles up by (`Manifest.title_lookup_namespaces`, as TMDB looks up `imdb` and `tvdb`), and it answers `unsupported_id` instead of failing when asked by an id it can't use. Each Library holds an ordered list of title sources (`title_sources`, `tmdb` by default) and asks only those. A search goes to each in turn, and the first to find anything wins. A source that fails stops the search as unavailable rather than passing it to the next, so a source being down never changes which source Matches. An id, whether a pin, an ID Hint or a fetch, goes first to the source keeping its namespace, then to those that look it up, and never to a source that does neither; a pin or ID Hint whose namespace no enabled source serves waits undecided, rather than being stored as Unmatched. A Show's Seasons come from the source that gave the Show, asked by the Show's id in that source's namespace. A title's preferred id follows the Library's order, replacing the fixed `tmdb`, `imdb`, `tvdb` preference of ADR 0042. Requests, which belong to no Library, route ids the same way and search in turn in the Admin's precedence of sources.

## Considered Options

- **One Extension Point per capability** (search, details, artwork): rejected. Many Plugins serve only part of a title source, and optional methods on one point let each serve what it has without a point per method.
- **The Kernel guessing which namespaces a source can use**: rejected. Only the source knows which ids its service accepts, so it declares them in its Manifest and says `unsupported_id` for the rest.

## Consequences

- Dropped without compatibility: the Kernel's fixed list of namespaces looked up across sources, the Rust title structs in `nama-plugin-api` (the contract messages carry what Plugins need), and fetching Seasons from the first source by Admin precedence.
- Changing a Library's order changes where new Matches come from; re-matching a Library when its order changes is left out.
- An Installed source that doesn't know a namespace answers `unsupported_id`, which the Kernel treats as not found there, not as the source being unavailable, so it isn't retried.
