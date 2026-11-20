# Sonarr responses for tests

Responses from the Sonarr v4 API (served under `/api/v3`), scripted in the Plugin's own tests
(`src/exports/tests.rs`), so no test needs a Sonarr. They follow Sonarr's documented response
shapes, trimmed to a realistic subset of fields, and were written by hand rather than captured,
because the factory that built the Plugin had no Sonarr. When re-recording one, trim it the same
way.

| File | Answers |
|---|---|
| `qualityprofile.json` | `GET /api/v3/qualityprofile` |
| `series-lookup.json` | `GET /api/v3/series/lookup?term=tvdb:334824` (Dark) while Sonarr doesn't have it |
| `series-exists.json` | `POST /api/v3/series` for a Show Sonarr already has: `400 Bad Request` |
| `episode.json` | `GET /api/v3/episode?seriesId=1`, Dark's Episodes, trimmed to two per Season |
| `queue.json` | `GET /api/v3/queue?includeSeries=true`, downloading Dark's Season 1 as a pack, listed once per Episode as Sonarr does, a Season 2 Episode queued, and two Season 3 Episodes, one of them blocked from import |

The tests script the rest inline: a Show Sonarr has (`GET /api/v3/series?tvdbId=`), its commands
(`GET /api/v3/command`), and `202 Accepted` to `POST /api/v3/seasonpass`,
`PUT /api/v3/series/editor` and `PUT /api/v3/episode/monitor`. A request without the right
`X-Api-Key` gets Sonarr's `401 Unauthorized`, with no body.
