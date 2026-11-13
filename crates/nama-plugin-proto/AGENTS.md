## nama-plugin-proto

- Never edit `src/generated/` by hand, and never let rustfmt touch it. Change the schema in `proto/nama/plugin/`, then run `buf generate` in `proto/`. See `README.md`.
- The contract imports nothing from the client API, `proto/nama/v1/` (ADR 0041). `tests/stands_alone.rs` checks it.
- A change under `proto/nama/plugin/` needs no entry in `proto/BREAKING.md`; `CONTRACT_VERSION` covers it.
- `CONTRACT_VERSION` in `src/lib.rs` changes with what an Installed Plugin sees, not with this crate's version in `Cargo.toml`. While it is 0.x, a change a Plugin built before can't work with bumps the minor, a change that only adds something a Plugin may use bumps the patch, and a change Plugins can't see bumps nothing. Changes that land on main together take one bump.
