-- Linked Users' changes to Watch State and to Subtitle and Audio Preferences waiting for the Hub,
-- which the Kernel sends now rather than the Hub Plugin (ADR 0008, ADR 0092, ADR 0105): kept while
-- the Hub is unreachable, so they go once it's back, in the order queued. The Hub keeps the change
-- made last of each title, whatever order they reach it in, so a title may wait more than once.
CREATE TABLE hub_sync_queue (
    -- Never reused, so a page being sent never names a change queued while it was.
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    -- 1 for a Subtitle or Audio Preference change, 0 for a Watch State change.
    preference INTEGER NOT NULL CHECK (preference IN (0, 1)),
    user_id INTEGER NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    -- The Hub API's `TrackPreferenceChange` or `WatchStateChange`, as its JSON.
    change TEXT NOT NULL
) STRICT;

-- Taken over from the Hub Plugin's Store, which kept each kind's queue as numbered pages, each a
-- JSON array of those same messages, and an older Plugin its Preferences as one array.
INSERT INTO hub_sync_queue (preference, user_id, change)
SELECT plugin_store.key NOT LIKE 'watch-state-queue/%',
       CAST(json_extract(queued.value, '$.userId') AS INTEGER),
       queued.value
FROM plugin_store,
     json_each(CASE WHEN json_valid(CAST(plugin_store.value AS TEXT))
                    THEN CAST(plugin_store.value AS TEXT) ELSE '[]' END) AS queued
WHERE plugin_store.plugin_id = 'hub' AND plugin_store.user_id IS NULL
  AND (plugin_store.key LIKE 'watch-state-queue/%' OR plugin_store.key LIKE 'track-preferences/%'
       OR plugin_store.key = 'track-preference-queue')
  AND queued.type = 'object'
  AND CAST(json_extract(queued.value, '$.userId') AS INTEGER) IN (SELECT id FROM users)
-- The older Plugin's one array came before any page, and its key has no page number to sort by.
ORDER BY plugin_store.key <> 'track-preference-queue',
         CAST(substr(plugin_store.key, instr(plugin_store.key, '/') + 1) AS INTEGER), queued.key;

DELETE FROM plugin_store
WHERE plugin_id = 'hub' AND user_id IS NULL
  AND (key IN ('watch-state-queue', 'track-preferences', 'track-preference-queue')
       OR key LIKE 'watch-state-queue/%' OR key LIKE 'track-preferences/%');
