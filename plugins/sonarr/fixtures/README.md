# Sonarr responses for tests

Responses from the Sonarr v4 API (served under `/api/v3`), served by the fake Sonarr in the
Server's tests (`crates/nama-server/tests/support/fake_sonarr.rs`), so no test needs a Sonarr. They
follow Sonarr's documented response shapes, trimmed to a realistic subset of fields, and were
written by hand rather than captured, because the factory that built the Plugin had no Sonarr.
When re-recording one, trim it the same way.

| File | Answers |
|---|---|
| `qualityprofile.json` | `GET /api/v3/qualityprofile` |
| `rootfolder.json` | `GET /api/v3/rootfolder` |
| `series-lookup.json` | `GET /api/v3/series/lookup?term=tvdb:334824` (Dark) while Sonarr doesn't have it |
| `series-exists.json` | `POST /api/v3/series` for a Show Sonarr already has: `400 Bad Request` |
| `episode.json` | `GET /api/v3/episode?seriesId=1`, Dark's Episodes, trimmed to two per Season |
| `command.json` | `POST /api/v3/command` that starts a `SeasonSearch`: `201 Created` |

The fake answers the rest from what it was sent: a lookup for a Show it has, or for one it can't
find (`[]`), the added Show, and `202 Accepted` to `POST /api/v3/seasonpass` and
`PUT /api/v3/episode/monitor`. A request without the right `X-Api-Key` gets Sonarr's
`401 Unauthorized`, with no body.
