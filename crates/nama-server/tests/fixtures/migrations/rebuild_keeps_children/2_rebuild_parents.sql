CREATE TABLE parents_new (id INTEGER PRIMARY KEY, name TEXT NOT NULL CHECK (name <> ''));
INSERT INTO parents_new (id, name) SELECT id, name FROM parents;
DROP TABLE parents;
ALTER TABLE parents_new RENAME TO parents;
