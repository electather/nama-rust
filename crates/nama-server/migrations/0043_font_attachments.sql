-- The fonts a Version's file carries, as a JSON array of each one's place among the file's
-- attachments, which is how ffmpeg picks it out. Files probed before it hold none until
-- PROBE_VERSION 7 probes them again.
ALTER TABLE version_probes ADD COLUMN font_attachments TEXT NOT NULL DEFAULT '[]';
