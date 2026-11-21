# TheIntroDB responses for tests

Responses from TheIntroDB's v4 API, read by the Plugin's unit tests so no test reaches TheIntroDB.
They were captured anonymously on 2026-11-20.

| File | Answers |
|---|---|
| `episode.json` | `GET /v4/media?tvdb_id=81189&season=1&episode=1&duration_ms=3480000`, Breaking Bad's first Episode |
| `movie.json` | `GET /v4/media?tmdb_id=550&duration_ms=8340000`, Fight Club, whose Intro starts at `null` |
| `unknown.json` | `GET /v4/media?tmdb_id=9999999&duration_ms=8340000`, a 404 for a title TheIntroDB doesn't know |
