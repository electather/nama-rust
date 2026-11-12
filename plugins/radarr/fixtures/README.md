# Radarr responses for tests

Responses from the Radarr v3 API, served by the fake Radarr in the Server's tests
(`crates/nama-server/tests/support/fake_radarr.rs`), so no test needs a Radarr. They follow
Radarr's documented response shapes, trimmed to a realistic subset of fields, and were written by
hand rather than captured, because the factory that built the Plugin had no Radarr. When
re-recording one, trim it the same way.

| File | Answers |
|---|---|
| `qualityprofile.json` | `GET /api/v3/qualityprofile` |
| `rootfolder.json` | `GET /api/v3/rootfolder` |
| `movie.json` | `POST /api/v3/movie` that adds the Movie: `201 Created` |
| `movie-exists.json` | `POST /api/v3/movie` for a Movie Radarr already has: `400 Bad Request` |
| `movie-not-found.json` | `POST /api/v3/movie` for a TMDB id Radarr can't find: `400 Bad Request` |
| `movie-alien.json` | `GET /api/v3/movie?tmdbId=348`, for Alien, which Radarr already has at Ultra-HD |
| `queue-empty.json` | `GET /api/v3/queue?includeMovie=true` with nothing downloading |
| `queue.json` | The same, downloading Heat, Alien's import blocked with a warning, and The Matrix queued |
| `queue-later.json` | The same, with Heat further along |

A request without the right `X-Api-Key` gets Radarr's `401 Unauthorized`, with no body. `GET /api/v3/movie?tmdbId=` for a Movie Radarr doesn't have gets `[]`, and `PUT /api/v3/movie/editor`, which unmonitors Movies, gets `202 Accepted` with `[]`.
