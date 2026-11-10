-- Whether the file marks a Track for the deaf and hard of hearing (SDH). Tracks read by probe
-- version 2 or older have 0 until probed again.
ALTER TABLE tracks ADD COLUMN sdh INTEGER NOT NULL DEFAULT 0;
