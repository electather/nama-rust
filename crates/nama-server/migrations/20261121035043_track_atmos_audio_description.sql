-- Whether an audio Track's profile says it carries Dolby Atmos, and whether its file marks it as
-- Audio Description. Tracks probed before read 0 until the probe pass reads them again, which
-- the `PROBE_VERSION` bump beside this migration has it do.
ALTER TABLE tracks ADD COLUMN atmos INTEGER NOT NULL DEFAULT 0 CHECK (atmos IN (0, 1));
ALTER TABLE tracks ADD COLUMN audio_description INTEGER NOT NULL DEFAULT 0
CHECK (audio_description IN (0, 1));
