-- Which Libraries each User sees: every one, those added later included, while `all_libraries`
-- is 1, else only those `user_libraries` lists, a chosen list that never grows by itself.
-- Deleting the User or the Library deletes its rows.
ALTER TABLE users ADD COLUMN all_libraries INTEGER NOT NULL DEFAULT 1 CHECK (all_libraries IN (0, 1));

CREATE TABLE user_libraries (
    user_id INTEGER NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    library_id INTEGER NOT NULL REFERENCES libraries (id) ON DELETE CASCADE,
    PRIMARY KEY (user_id, library_id)
) STRICT, WITHOUT ROWID;

CREATE INDEX user_libraries_by_library ON user_libraries (library_id);
