-- The size and modified time of the file each Version was probed at, so a file changed since is
-- probed again. Rows probed before these existed get a size of -1, which no file has, so they're
-- probed again too.
ALTER TABLE version_probes ADD COLUMN size INTEGER NOT NULL DEFAULT -1;
ALTER TABLE version_probes ADD COLUMN modified_ns INTEGER NOT NULL DEFAULT -1;
