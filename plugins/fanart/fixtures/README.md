# fanart.tv responses for tests

Responses from the fanart.tv v3 API, read by the Plugin's unit tests so no test reaches fanart.tv.
They follow fanart.tv's documented v3 response shapes, trimmed to a few images per list, and were
written by hand rather than captured, because the factory that built the Plugin had no fanart.tv
API key. When re-recording one, trim it the same way.

| File | Answers |
|---|---|
| `movie.json` | `GET /v3/movies/949`, Heat, with an `hdmovielogo` in `en` and one in `00` |
| `tv.json` | `GET /v3/tv/371980`, Severance, with a season-level list the Plugin skips |
