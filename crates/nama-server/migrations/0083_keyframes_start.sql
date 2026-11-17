-- When a Version's file starts, in milliseconds, as Probing read it with `keyframes_ms`: a Remux
-- cuts copied audio by it in a file that starts late, such as an MPEG-TS (#944). NULL beside
-- keyframes listed before it was kept, which are listed again.
ALTER TABLE version_probes ADD COLUMN keyframes_start_ms INTEGER;
