# Vite+ is the web app's only toolchain

The web app uses Vite+ (`vp`) for everything: pinning Node and pnpm, installing, the dev server, the build, formatting (oxfmt), linting (oxlint), type-checking (tsgo through oxlint's type-aware mode) and tests (Vitest). All of it is configured in one `vite.config.ts`. The only tools outside it are Fallow (unused code and dependencies, duplication, complexity, import boundaries), `protoc-gen-es` for codegen, and impeccable. Vite+ replaces the usual mix of tools and config files, and its Rust-based tools are fast enough to run strict checks on every change, as the Rust side does.

## Considered Options

- **Node + pnpm + Vite + ESLint + Prettier + tsc + Vitest, configured separately**: rejected. That's five configs to keep in step, and slower checks for the same strictness.
- **Biome for lint and format**: rejected once Vite+ was chosen, because Vite+ already ships oxlint and oxfmt.

## Consequences

- Vite+ is young and comes from one company. If it stalls, its parts (Vite, Vitest, oxlint, oxfmt) work on their own, and the move is a config change, not a rewrite.
- TanStack Start has to run on Vite+'s Vite. The web app shell (#28) proves `vp dev` and `vp build` with TanStack Start in SPA mode first.
- Lint rules are oxlint's. Rules that exist only as ESLint plugins (for example `@tanstack/eslint-plugin-query`) aren't available.
