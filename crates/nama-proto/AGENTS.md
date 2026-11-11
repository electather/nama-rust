## nama-proto

- Never edit `src/generated/` by hand, and never let rustfmt touch it. Change the schema in `proto/` at the workspace root, then run `buf generate` there. See `README.md`.
- A change that breaks the schema on purpose adds a new entry to `proto/BREAKING.md` (ADR 0032). See `README.md`.
