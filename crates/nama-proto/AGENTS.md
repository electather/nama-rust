## nama-proto

- Never edit `src/generated/` by hand, and never let rustfmt touch it. Change the schema in `proto/` at the workspace root, then run `buf generate` there. See `README.md`.
