-- When each keyframe of a Version's first video falls, in milliseconds, as a JSON array: listed
-- the first time the Version is Remuxed and kept for the next. It goes with its row, so a file
-- probed again has them listed again.
ALTER TABLE version_probes ADD COLUMN keyframes_ms TEXT;
