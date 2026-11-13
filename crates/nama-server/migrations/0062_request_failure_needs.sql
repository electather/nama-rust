-- The id namespaces its Fulfiller needed and the title had none of, comma-separated (`tvdb`), so
-- the client API can say why the Request failed (ADR 0042). Only on a Request the Kernel failed
-- for that without handing it off; NULL on every other.
ALTER TABLE requests ADD COLUMN failure_needs TEXT;
