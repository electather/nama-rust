## web/src

The app is organised by feature (ADR 0064). Code is moving into this layout (#1132). Until it has, the old folders are **legacy**: they keep the rules they had, and a file meets the rules below once it moves.

- **Layout.**
  - `app/`: the shell: providers, layouts and the app chrome (`app/AGENTS.md`).
  - `features/<feature>/`: one folder per feature. `features/AGENTS.md` says what goes in one.
  - `shared/ui/`: the shadcn components, which are our code (`shared/ui/AGENTS.md`).
  - `shared/components/`: components more than one feature renders.
  - `shared/hooks/`: hooks more than one feature calls.
  - `shared/lib/`: functions, types and constants with no React in them.
  - `routes/`: TanStack Router's file routes, which are thin (`routes/AGENTS.md`).
  - `gen/` and `routeTree.gen.ts`: generated (`web/AGENTS.md`).
  - Legacy: `components/`, `lib/` and the flat `*.test.tsx` here. New code goes in the folders above.
- **Layers.** Imports go one way, which Fallow's zones enforce (`.fallowrc.jsonc`):
  - `shared/lib/` imports only `shared/lib/`.
  - `shared/hooks/` imports `shared/hooks/` and `shared/lib/`.
  - `shared/ui/` imports only `shared/ui/`.
  - `shared/components/` imports the other `shared/` folders.
  - A feature imports `shared/` and the sibling features its zone lists.
  - `app/` imports `shared/` and the features it renders.
  - `routes/` imports everything but another route file. Only `main.tsx`, `router.tsx` and the route tree import route files.

  While the move runs, every zone may also import the legacy folders. #1150 removes that.

- **One component per file**, small stateless helpers included, so a file's name says what it renders. `shared/ui/` is the exception: it keeps shadcn's files as shadcn writes them. A component file exports only components, so Fast Refresh keeps its state. A test may keep wrapper and fixture components inline. `vp check` enforces both.
- **File size.** A file holds at most 200 lines, and a test in `__tests__/` 400. Fallow enforces it. Today's longer legacy files are listed in `.fallowrc.jsonc`, and a file leaves that list when it moves.
- **Import from the defining file.** Each import names the file that defines the thing, through the `@/` alias, never a parent-relative path. An `index.ts` that re-exports is a barrel: `vp check` fails on `export *`, but a file of named re-exports (`export { A } from "./a"`) is unchecked.
- **Data hooks.** Only `app/data/`, a feature's `data/` files and `shared/hooks/` import `@connectrpc/connect-query` or `@tanstack/react-query`. A component calls a data hook, so it never builds a query itself. Fallow's `feature-layout/data-hooks` rule enforces it.
- **Suspense.** Read with `useSuspenseQuery`, so a Suspense boundary shows the loading state. A query that genuinely mustn't suspend uses `useQuery` behind `// fallow-ignore-next-line policy-violation:feature-layout/suspense -- <reason>`. Fallow's `feature-layout/suspense` rule enforces it.
- **Dynamic loading.** `hls.js`, `libbitsub`, `jsqr` and the ASS renderer load with `import()` where they're needed, so the first page stays small. A type-only import is fine. Fallow's `feature-layout/dynamic-loading` rule enforces it outside tests.
- **Tests live in `__tests__/`** beside the code they test, importing it through `@/`. Vitest runs only those, and the legacy flat tests here. A test file anywhere else never runs, and Fallow reports it as unused.
- **Strictest lint.** The new folders get extra oxlint rules (the `newFolders` override in `vite.config.ts`). The ones that change how code is written:
  - `satisfies` or a type guard, never a cast.
  - `""` and `0` aren't conditions: compare them. Checking whether an object is there is fine.
  - A function that returns a promise is `async`, and awaits rather than chaining `.then`.
  - At most 3 parameters, and JSX nested at most 2 deep, so a component that grows splits.
  - Named regex capture groups, explicit `public`/`private`, `globalThis`, no `reduce`, and no member access straight off an `await`.
  - Tests compare with `toStrictEqual` and keep setup inside `describe` or the hook functions.
