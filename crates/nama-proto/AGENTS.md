## nama-proto

- Never edit `src/generated/` by hand, and never let rustfmt touch it. Change the schema in `proto/` at the workspace root, then run `buf generate` there. See `README.md`.
- A change that breaks the schema on purpose adds a new entry to `proto/BREAKING.md` (ADR 0032). See `README.md`.
- `NAMA_API_VERSION` in `src/lib.rs` changes with what an Installed Plugin sees, not with this crate's version in `Cargo.toml`. While it is 0.x, a change a Plugin built before can't work with bumps the minor, a change that only adds something a Plugin may use bumps the patch, and a change Plugins can't see bumps nothing.
