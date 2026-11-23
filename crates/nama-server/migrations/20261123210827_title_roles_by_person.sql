-- A Person's page reads every Role of theirs.
CREATE INDEX title_roles_by_person ON title_roles (person_namespace, person_value);
