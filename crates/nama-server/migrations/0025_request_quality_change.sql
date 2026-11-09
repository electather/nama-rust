-- The latest change a Request manager made to a Request's Quality, all NULL if none: only the
-- latest is kept. Who made it is forgotten when that User is deleted, as their Requests are.
ALTER TABLE requests ADD COLUMN quality_changed_by INTEGER REFERENCES users (id) ON DELETE SET NULL;
-- The friendly name of the Quality it had before, NULL if it had none.
ALTER TABLE requests ADD COLUMN quality_changed_from TEXT;
ALTER TABLE requests ADD COLUMN quality_changed_at INTEGER;
