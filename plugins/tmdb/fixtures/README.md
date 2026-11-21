# TMDB responses for tests

Responses from the TMDB v3 API, served by `nama_plugin_tmdb::testing::FakeTmdb` so no test
touches the network. `routes.json` maps each request (path, then its query parameters sorted by
name, leaving out `api_key`, `language` and `include_adult`) to the file under `api/` that
answers it. Images lists (`/movie/{id}/images`, `/tv/{id}/images`,
`/tv/{id}/season/{n}/images`) are asked in every language, so their routes have no query. A request with no route gets TMDB's own 404 body. `images/` holds stand-in image
bytes, served under `/t/p/<size>/`.

The bodies follow TMDB's documented response shapes and are trimmed to a realistic subset of
fields. They were written by hand rather than captured, because the factory that built the
plugin had no TMDB API key. When re-recording one, keep its route and trim it the same way.

`tv-70523-season-3.json` departs from TMDB on purpose: its second Episode airs in 2099, so tests
have a Season still airing.

`search-tv-euphoria.json` departs from TMDB on purpose too: its second Euphoria, with no
`origin_country`, is made up, so tests have a Show a country tag drops.

`tv-2316.json` departs from TMDB on purpose: the US Office has only Seasons 1 and 3, so a Season 3
Episode fits it and not the UK Office, which ties with it in a search.

`search-tv-unreadable.json` is made up: its `results` isn't a list, so tests have a Show search
TMDB answers with a page the Plugin can't read.

`tv-900001.json` and its Season are made up: a Show with neither a TVDB nor an IMDb id, so tests
have a Show a Fulfiller needing one cannot take.

The images lists depart from TMDB on purpose: each is built so the artwork pick rule (English, then
textless, then other languages, but textless first for backdrops; then rating) takes the title's
own `poster_path` and `backdrop_path`, since those are the only images whose bytes are here,
besides Alien's and Dark Season 1's second English posters, which tests pick as Custom Images. Heat has no logos.
`wire_tests.rs` checks this; the picked logos' bytes are 1x1 PNGs.

Only Alien (`movie-348.json`) and Dark (`tv-70523.json`) have credits, trimmed to a few people; a Producer among Alien's crew, and Dark's Executive Producer, are jobs the Plugin leaves out. The headshots of those billed first are among `images/`, as stand-in bytes.

`movie-949.json` has no US release, so tests have a title with no rating in the Language
setting's country.
