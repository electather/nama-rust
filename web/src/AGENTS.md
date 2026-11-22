## web/src

The web app's own rules. `web/AGENTS.md` has the ones every web package shares. The app is organised by feature (ADR 0064), and its components are built from nama-ui's (`web/ui/`).

- **Layout.**
  - `app/`: the shell: providers, layouts and the app chrome (`app/AGENTS.md`).
  - `features/<feature>/`: one folder per feature. `features/AGENTS.md` says what goes in one.
  - `shared/components/`: components more than one feature renders.
  - `shared/hooks/`: hooks more than one feature calls.
  - `shared/lib/`: functions, types and constants with no React in them.
  - `routes/`: TanStack Router's file routes, which are thin (`routes/AGENTS.md`).
  - `gen/` and `routeTree.gen.ts`: generated (below).
  - `test-*.ts(x)`: the fake Server and helpers the tests share.
  - `styles.css`: the app's global CSS. It imports nama-ui's tokens and fonts, and the tokens of Home and the title pages from `home.css` and `title.css`. Text subtitles' `::cue` rules are in `subtitle-cues.css`.
- **Layers.** Imports go one way, which Fallow's zones enforce (`.fallowrc.jsonc`):
  - `shared/lib/` imports only `shared/lib/`.
  - `shared/hooks/` imports `shared/hooks/` and `shared/lib/`.
  - `shared/components/` imports the other `shared/` folders.
  - A feature imports `shared/` and the sibling features its zone lists.
  - `app/` imports `shared/` and the features it renders.
  - `routes/` imports everything but another route file. Only `main.tsx`, `router.tsx` and the route tree import route files.
- **Never edit `gen/` or `routeTree.gen.ts` by hand.** `gen/` imports nothing from the app. Change `proto/` and run `buf generate` there, or let the router plugin regenerate the route tree.
- **Sensitive changes ask "Confirm it’s you".** Call an RPC that needs Recent Authentication (`crates/nama-server/AGENTS.md`) with `useSensitiveMutation`, never connect-query's `useMutation`. `web/README.md` says how it works.
- **Settings, not Account.** A User's own settings are "Settings" (`?settings=<section>`, a modal over any signed-in page), never "Account": an Account is the Hub identity. Don't use `/link` for anything but Linking an Account.
- **One component per file**, small stateless helpers included, so a file's name says what it renders. A component file exports only components, so Fast Refresh keeps its state. A test may keep wrapper and fixture components inline. `vp check` enforces both.
- **File size.** A file holds at most 200 lines, and a test in `__tests__/` 400. Fallow enforces it.
- **Import from the defining file.** Each import names the file that defines the thing, through the `@/` alias, or `nama-ui/components/<file>`, never a parent-relative path. An `index.ts` that re-exports is a barrel: `vp check` fails on `export *`, but a file of named re-exports (`export { A } from "./a"`) is unchecked.
- **Data hooks.** Only `app/data/`, a feature's `data/` files and `shared/hooks/` import `@connectrpc/connect-query` or `@tanstack/react-query`. A component calls a data hook, so it never builds a query itself. Fallow's `feature-layout/data-hooks` rule enforces it.
- **Loader data.** A hook that reads a query a route's loader or access check prefetches passes `staleTime: loaderStaleTime` (`shared/hooks/loader-stale-time.ts`). Without it, the hook asks the Server again whenever the page mounts more than 1 s after its loader, and a plain `useQuery` asks on every mount.
- **Suspense.** Read with `useSuspenseQuery`, so a Suspense boundary shows the loading state. A query that genuinely mustn't suspend uses `useQuery` behind `// fallow-ignore-next-line policy-violation:feature-layout/suspense -- <reason>`. Fallow's `feature-layout/suspense` rule enforces it.
- **Dynamic loading.** `hls.js`, `libbitsub`, `jsqr` and the ASS renderer load with `import()` where they're needed, so the first page stays small. A type-only import is fine. Fallow's `feature-layout/dynamic-loading` rule enforces it outside tests.
- **Tests live in `__tests__/`** beside the code they test, importing it through `@/`. Vitest runs only those. A test file anywhere else never runs, and Fallow reports it as unused.
- **Strictest lint.** `app/`, `features/` and `shared/`, and nama-ui, get extra oxlint rules (the `newFolders` override in `vite.config.ts`). The ones that change how code is written:
  - `satisfies` or a type guard, never a cast.
  - `""` and `0` aren't conditions: compare them. Checking whether an object is there is fine.
  - A function that returns a promise is `async`, and awaits rather than chaining `.then`.
  - At most 3 parameters, and JSX nested at most 2 deep, so a component that grows splits.
  - Named regex capture groups, explicit `public`/`private`, `globalThis`, no `reduce`, and no member access straight off an `await`.
  - Tests compare with `toStrictEqual` and keep setup inside `describe` or the hook functions.
