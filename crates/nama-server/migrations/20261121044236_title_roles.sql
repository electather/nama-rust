-- A Movie's or Show's Roles, kept once per title like its Score rather than copied onto each of
-- its Matches, so a Show's aren't stored on every Episode's (ADR 0081). Keyed as `title_scores`
-- is. A row outlives its Matches, as other per-title data does.
CREATE TABLE title_roles (
    plugin TEXT NOT NULL,
    kind TEXT NOT NULL CHECK (kind IN ('movie', 'show')),
    namespace TEXT NOT NULL,
    value TEXT NOT NULL,
    -- Crew or Cast, and the Role's place in that list's billing order, from 0.
    crew INTEGER NOT NULL CHECK (crew IN (0, 1)),
    billed INTEGER NOT NULL CHECK (billed >= 0),
    -- The Person's External ID.
    person_namespace TEXT NOT NULL,
    person_value TEXT NOT NULL,
    name TEXT NOT NULL,
    -- The character, or the job, such as 'Director'.
    part TEXT NOT NULL,
    -- The headshot's path at the Plugin's title source, kept only for the first 15 of the Cast.
    -- The image is stored once per path, so a Person's is shared by all their Roles.
    headshot TEXT,
    PRIMARY KEY (plugin, kind, namespace, value, crew, billed)
) STRICT, WITHOUT ROWID;

-- Serving a headshot asks whether some Role has it.
CREATE INDEX title_roles_by_headshot ON title_roles (plugin, headshot) WHERE headshot IS NOT NULL;
