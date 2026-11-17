CREATE TABLE parents (id INTEGER PRIMARY KEY, name TEXT NOT NULL);
CREATE TABLE children (
    id INTEGER PRIMARY KEY,
    parent_id INTEGER NOT NULL REFERENCES parents (id) ON DELETE CASCADE
);
INSERT INTO parents (id, name) VALUES (1, 'a'), (2, 'b');
INSERT INTO children (id, parent_id) VALUES (1, 1), (2, 1), (3, 2);
