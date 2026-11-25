## nama-hub-proto

- Never edit `src/generated/` by hand, and never let rustfmt touch it. Change the schema in `proto/nama/hub/`, then run `buf generate` in `proto/` (`crates/nama-proto/README.md`).
- A change that breaks the Hub API (`proto/nama/hub/v1/`) on purpose adds a new entry to `proto/BREAKING.md` naming `nama.hub.v1` (ADR 0032): Servers already deployed keep calling it.
