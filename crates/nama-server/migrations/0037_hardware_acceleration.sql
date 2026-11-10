-- What Transcodes run on: '' for Automatic, 'off', or an API such as 'vaapi'.
ALTER TABLE server_settings ADD COLUMN hardware_acceleration TEXT NOT NULL DEFAULT '';

-- The Transcode cap becomes NULL for the default, which is no limit while Transcodes use hardware
-- acceleration and 2 while they run in software. A saved 2 can't be told from the old default, so
-- it becomes the default too.
ALTER TABLE server_settings ADD COLUMN transcode_cap INTEGER;
UPDATE server_settings SET transcode_cap = max_transcodes WHERE max_transcodes <> 2;
ALTER TABLE server_settings DROP COLUMN max_transcodes;
ALTER TABLE server_settings RENAME COLUMN transcode_cap TO max_transcodes;
