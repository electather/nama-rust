# TVDB responses for tests

Responses from the TVDB v4 API for one Show, Severance (TVDB id 371980), read by the Plugin's unit
tests so no test reaches TVDB. They follow TVDB's documented response shapes, trimmed to the fields
the Plugin reads, and were written by hand rather than captured, because the factory that built the
Plugin had no TVDB API key. When re-recording one, trim it the same way.

| File | Answers |
|---|---|
| `login.json` | `POST /v4/login` |
| `search.json` | `GET /v4/search?query=severance&type=series`, with a second Show TVDB has only a Korean name and an English translation for |
| `series-extended.json` | `GET /v4/series/371980/extended?meta=translations`, trimmed to three artworks, two genres and the Seasons of two orders |
| `episodes-season-1.json` | `GET /v4/series/371980/episodes/default?season=1`, trimmed to two Episodes |
