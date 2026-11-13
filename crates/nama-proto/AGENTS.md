## nama-proto

- Never edit `src/generated/` by hand, and never let rustfmt touch it. Change the schema in `proto/` at the workspace root, then run `buf generate` there. See `README.md`.
- A change that breaks the client API (`proto/nama/v1/`) on purpose adds a new entry to `proto/BREAKING.md` (ADR 0032). See `README.md`. The Plugin contract, `proto/nama/plugin/`, has its own crate and rules: `crates/nama-plugin-proto/AGENTS.md`.
