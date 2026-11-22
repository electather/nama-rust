-- What Wikidata says about each title that its vector is built from (ADR 0095): an item, such as
-- a person or a genre, and what it is to the title. What may be stored here:
-- `src/titles/AGENTS.md`.
CREATE TABLE title_facts (
    id uuid PRIMARY KEY DEFAULT uuidv7(),
    title_id uuid NOT NULL REFERENCES titles (id) ON DELETE CASCADE,
    property text NOT NULL CHECK (
        property IN (
            'cast', 'director', 'writer', 'genre', 'subject', 'award', 'franchise', 'country'
        )
    ),
    -- Its Wikidata item, such as `Q9545711`.
    item text NOT NULL
);

-- So deleting a title, as an import does, doesn't read every fact.
CREATE INDEX title_facts_by_title ON title_facts (title_id);

-- Each run that built the title vectors, which each vector records, the way a Match records its
-- metadata version (ADR 0040, 0095).
CREATE TABLE training_runs (
    id uuid PRIMARY KEY DEFAULT uuidv7(),
    trained_at timestamptz NOT NULL DEFAULT now()
);

-- A title's vector, from the run that made it. A run replaces every vector in one transaction.
CREATE TABLE title_vectors (
    id uuid PRIMARY KEY DEFAULT uuidv7(),
    title_id uuid NOT NULL UNIQUE REFERENCES titles (id) ON DELETE CASCADE,
    training_run_id uuid NOT NULL REFERENCES training_runs (id),
    embedding vector(256) NOT NULL
);
