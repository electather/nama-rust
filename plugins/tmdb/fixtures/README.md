# TMDB responses for tests

Responses from the TMDB v3 API, served by `nama_plugin_tmdb::testing::FakeTmdb` so no test
touches the network. `routes.json` maps each request (path, then its query parameters sorted by
name, leaving out `api_key`, `language` and `include_adult`) to the file under `api/` that
answers it. A request with no route gets TMDB's own 404 body. `images/` holds stand-in image
bytes, served under `/t/p/<size>/`.

The bodies follow TMDB's documented response shapes and are trimmed to a realistic subset of
fields. They were written by hand rather than captured, because the factory that built the
plugin had no TMDB API key. When re-recording one, keep its route and trim it the same way.

`tv-70523-season-3.json` departs from TMDB on purpose: its second Episode airs in 2099, so tests
have a Season still airing.
