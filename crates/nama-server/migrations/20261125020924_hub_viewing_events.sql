-- Linked Users' Viewing Events wait for the Hub beside their Watch State and Preference changes,
-- now that the Kernel sends them rather than the Hub Plugin (ADR 0093, ADR 0105). A third kind
-- needs a CHECK SQLite can't alter, so the table is rebuilt; it has no index or trigger to recreate.
CREATE TABLE hub_sync_queue_new (
    -- Never reused, so a page being sent never names a change queued while it was.
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    -- 0 for a Watch State change, 1 for a Subtitle or Audio Preference change, 2 for a Viewing
    -- Event.
    kind INTEGER NOT NULL CHECK (kind IN (0, 1, 2)),
    user_id INTEGER NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    -- The Hub API's `WatchStateChange`, `TrackPreferenceChange` or `UserViewingEvent`, as its JSON.
    change TEXT NOT NULL
) STRICT;

INSERT INTO hub_sync_queue_new (id, kind, user_id, change)
SELECT id, preference, user_id, change FROM hub_sync_queue;

DROP TABLE hub_sync_queue;

ALTER TABLE hub_sync_queue_new RENAME TO hub_sync_queue;

-- Taken over from the Hub Plugin's Store, which kept them as numbered pages, each a JSON array of
-- `UserViewingEvent`.
INSERT INTO hub_sync_queue (kind, user_id, change)
SELECT 2, CAST(json_extract(queued.value, '$.userId') AS INTEGER), queued.value
FROM plugin_store,
     json_each(CASE WHEN json_valid(CAST(plugin_store.value AS TEXT))
                    THEN CAST(plugin_store.value AS TEXT) ELSE '[]' END) AS queued
WHERE plugin_store.plugin_id = 'hub' AND plugin_store.user_id IS NULL
  AND plugin_store.key LIKE 'viewing-event-queue/%'
  AND queued.type = 'object'
  AND CAST(json_extract(queued.value, '$.userId') AS INTEGER) IN (SELECT id FROM users)
ORDER BY CAST(substr(plugin_store.key, instr(plugin_store.key, '/') + 1) AS INTEGER), queued.key;

-- `viewing-events` is what a Plugin of 22 November 2026 kept, as protobuf SQL can't read; any later
-- Plugin moved it into its pages within five minutes of starting.
DELETE FROM plugin_store
WHERE plugin_id = 'hub' AND user_id IS NULL
  AND (key IN ('viewing-event-queue', 'viewing-events') OR key LIKE 'viewing-event-queue/%');
