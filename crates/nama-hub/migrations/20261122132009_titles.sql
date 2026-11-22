-- Films and series from Wikidata's dump, the Hub's External ID crosswalk (ADR 0095). What may be
-- stored here: `src/titles/AGENTS.md`.
CREATE TABLE titles (
    id uuid PRIMARY KEY DEFAULT uuidv7(),
    -- Its Wikidata item, such as `Q83495`, which each import matches the row on.
    wikidata_id text NOT NULL UNIQUE,
    kind text NOT NULL CHECK (kind IN ('movie', 'show')),
    -- When the last import that found it ran: an import deletes the titles it didn't find.
    imported_at timestamptz NOT NULL DEFAULT now()
);

-- A title's External IDs, at most one per namespace (ADR 0042).
CREATE TABLE title_ids (
    id uuid PRIMARY KEY DEFAULT uuidv7(),
    title_id uuid NOT NULL REFERENCES titles (id) ON DELETE CASCADE,
    namespace text NOT NULL,
    value text NOT NULL,
    UNIQUE (title_id, namespace)
);

CREATE INDEX title_ids_by_value ON title_ids (namespace, value);
