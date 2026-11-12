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
| `queue-empty.json` | `GET /api/v3/queue?includeSeries=true` with nothing downloading |
| `queue.json` | The same, downloading Dark's Season 1 as a pack, listed once per Episode as Sonarr does, a Season 2 Episode queued, and two Season 3 Episodes, one of them blocked from import |
| `queue-later.json` | The same, with the Season 1 pack further along |

The fake answers the rest from what it was sent: its Shows by TVDB id (`GET /api/v3/series?tvdbId=`),
a lookup for a Show it can't find (`[]`), the added Show, its commands (`GET /api/v3/command`), and
`202 Accepted` to `POST /api/v3/seasonpass`, `PUT /api/v3/series/editor` and
`PUT /api/v3/episode/monitor`, which monitors or unmonitors Episodes. `GET /api/v3/episode` without
`seasonNumber` lists every Season's Episodes. Adding a Show
starts a `RefreshSeries` that, as Sonarr's does, saves the Seasons back as it read them when it
started, and a Show has Episodes only once it finishes. A request without the right `X-Api-Key`
gets Sonarr's `401 Unauthorized`, with no body.
