-- Whether the file carries a subtitle Track's codec header, such as VobSub's `.idx` with its
-- palette and size. NULL for a Track that isn't a subtitle, and for one probed before it, until
-- PROBE_VERSION 8 probes its file again.
ALTER TABLE tracks ADD COLUMN header INTEGER;
