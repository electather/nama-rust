# Jobs in Jellyfin, nama-stream and Silo

Report, 2026-11-20. Three analysts read shallow clones of `jellyfin/jellyfin` (master d88449a), `electather/nama-stream` and `Silo-Server/silo-server`, for issue #1005.

## At a glance

| | Jellyfin | nama-stream | Silo |
|---|---|---|---|
| Declared as | `IScheduledTask` class, found by reflection at startup | `register*()` call per domain; plugin Manifest `jobs: [...]` | `Task` interface + optional capability interfaces; plugin `scheduled_task.v1` capability |
| Plugins vs core | Identical | Same runner, id `plugin.<plugin>.<job>` | Same runner, key `plugin:<install>:<capability>`, re-synced every minute |
| Triggers | daily, weekly, interval, startup (+ per-trigger max runtime) | cron (croner) | interval, daily, weekly, startup (+ max runtime, start spread) |
| Kinds | one | scheduled, per-row (per User/connection), triggerable with input, coalesced (debounced) | scheduled, manual-only, conditional (`ShouldRun`), library-scoped; separate durable admin-job queue |
| Admin edits | Replace trigger list; edit freezes defaults forever | Sparse override row: enabled, schedule, log level; absent row = code default | Edit triggers with ETag; plugin tasks follow defaults until edited |
| Same job twice | No (queued; manual run on busy task throws) | No (`skipped` row on tick, error on manual) | No (`ErrTaskAlreadyRunning`) |
| Different jobs at once | Unbounded | Unbounded | Unbounded; durable jobs use `SKIP LOCKED` |
| Progress | `IProgress<double>` over websocket | None live; per-row counts at the end | `Report(pct, msg)` over realtime event |
| Cancel | Token; timeout recorded as Cancelled | Cooperative abort signal; `cancelled`/`timed_out` | Context cancel with cause |
| Retries | None | None (features build their own) | None for tasks; durable jobs requeue on stale heartbeat |
| History | Last result only | `job_runs`: last 50 successes, failures forever, captured logs | `task_executions`: 30 days / 1000 per task, newest always kept |
| Restart mid-run | Recorded `Aborted` on clean shutdown; nothing on kill | Boot flips `running` rows to `failed` | Run lost, no row |

## Typical jobs

- **Library**: scan all Libraries, refresh metadata, refresh people, chapter images, trickplay/preview frames, segment (intro) detection, keyframes, missing subtitles.
- **Maintenance / retention**: clean cache, transcode and log folders; prune activity log, sessions, expired auth/OAuth rows, diagnostics, task history, orphaned images, plugin store expiry; database optimise (Silo folds many into one daily task with per-step status).
- **Outside services**: refresh discover/trending lists, sync watchlists and watch history from plugins, check plugin updates, guide/channel refresh.
- **Per-User**: rebuild recommendations, warm home layouts for recently active Users (nama-stream, 14 days).

## Lessons

- All three run plugin and core jobs through one runner, list and admin page.
- Defaults belong in code; store only the Admin's overrides, so a changed default still reaches untouched jobs (Jellyfin's frozen defaults are the trap).
- Keep a run row while running and close orphans at boot, or crashes leave no trace.
- Record skipped and partial-failure outcomes distinctly; record a timeout as a timeout, not a cancel.
- No product bounds concurrency across different jobs; heavy jobs overlapping is Jellyfin's top complaint.
- Retries are left to the next scheduled run everywhere.
- Event-as-job (nama-stream) and a second durable queue (Silo) both added complexity their authors flag.
