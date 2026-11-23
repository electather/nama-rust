-- A Movie Wikidata lacks gets a vector from Viewing Events alone (ADR 0095), under the External
-- ID Servers pool it by, such as `tmdb` `603`, as it has no `titles` row. Only Movies are pooled,
-- so such a vector is a Movie's.
ALTER TABLE title_vectors
    ALTER COLUMN title_id DROP NOT NULL,
    ADD COLUMN namespace text,
    ADD COLUMN value text,
    ADD UNIQUE (namespace, value),
    ADD CHECK (
        (title_id IS NOT NULL AND namespace IS NULL AND value IS NULL)
        OR (title_id IS NULL AND namespace IS NOT NULL AND value IS NOT NULL)
    );
