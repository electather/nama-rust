# Artwork comes from every ordered source's list

ADR 0044 left the artwork list for later, so a Match stored only the Poster and Backdrop of the source that Matched it, and a source with better artwork, such as fanart.tv, had no way in. A title source may now serve an artwork list, the optional `Artwork` rpc of `TitleSourceService` 1.1.0: every image it has for one title, of every kind, each with its language, rating and votes. When Matching stores a Match, Artwork asks every source in the Library's `title_sources` order that answers that method, by the title's id in a namespace the source keeps or looks up. It picks the Poster, Backdrop and Title Logo per kind: by language first (the asked language, a fixed `en` for now, then images with none, then any other), then by the source's place in the order, then by higher rating, then more votes. A kind no list has falls back to the Matching source's own Poster and Backdrop. Each image is stored with the Plugin whose list gave it, and served and downloaded under that Plugin. A source whose list fails is skipped and logged; it never makes a Match undecided. Ids route only to sources that answer the method being asked, so a list-only source like fanart.tv, which looks titles up by `tmdb`, `imdb` and `tvdb` but finds no Movie or Show, is never asked for a pin; this fixes ADR 0044's rule that a pin no source serves waits undecided, which such a source broke by answering it as not found. List-only sources share the Library's one order, and an order in which no source searches is refused, since it could never Match. `METADATA_VERSION` 2 refreshes existing Matches so they pick their artwork this way.

## Considered Options

- **A separate artwork source order per Library**: rejected. It doubles a Library's source settings for a rare choice; one order already says which sources a Library trusts first, for titles and artwork alike.
- **Picking per User at browse time**: rejected. A Match and its stored images are shared by every User, so the pick is made once when the Match is stored; a User's own language waits for per-User settings to be asked of it.

## Consequences

- Dropped without compatibility: a Match's images always being from its own Plugin. Browse serves each image under the Plugin stored beside it.
- A Library gains artwork from a source only by putting it in its order; a list-only source placed first wins every kind it has in the picked language.
- Matches made before `METADATA_VERSION` 2 are refreshed, which asks each source's list once per title.
