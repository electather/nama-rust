## web

Rules for every web package: the web app at the root, nama-ui, the design system, in `ui/`, and the Hub web app in `hub/` (`README.md`, "Toolchain"). The web app's own rules are in `src/AGENTS.md`, nama-ui's in `ui/src/components/AGENTS.md`.

- **Load the `web-app` skill** before you change anything here. It holds building to `PRODUCT.md` and `DESIGN.md` with impeccable, the checks, the screenshot step and the review.
- **Only `vp`.** Run, build, check and test through `vp` (ADR 0017). Don't add ESLint, Prettier, Biome, a separate `vitest.config.ts` or `.oxlintrc`. Config goes in `vite.config.ts`. An ESLint plugin oxlint loads as a JS plugin is fine: it runs inside `vp check`.
- **Suppressions name a reason.** To silence a lint, use `// oxlint-disable-next-line <rule> -- <reason>` on the line, never a whole file. For a type error, use `// @ts-expect-error <reason>`, never `@ts-ignore`. To raise a complexity limit, use a Fallow `thresholdOverrides` entry with a `reason`.
- **Semantic tokens only.** Style with shadcn's semantic utilities (`bg-background`, `text-muted-foreground`, `bg-primary`). Never use Tailwind palette colours, raw colour values or `dark:` variants (ADR 0016). If a screen needs a colour that no token covers, add a token to `ui/src/styles.css`.
- **Logical properties.** Use `ms-`/`me-`/`ps-`/`pe-`/`start-`/`end-`, never `ml-`/`mr-`/`pl-`/`pr-`/`left-`/`right-`, so right-to-left languages can be added later.
- **Imports go one way.** nama-ui imports nothing from an app, and an app imports it by name (`nama-ui/components/button`), never by path. `ui/tsconfig.json` leaves out the web app's `@/` alias, and lint bans parent-relative imports, so `vp check` fails on either. The two apps share nothing but nama-ui (ADR 0101): the Hub web app imports its own files through `@hub/`, never the web app's `@/`, and Fallow's `hub` zone and `hub/tsconfig.json` fail either reaching across.
- **The Hub web app speaks only `nama.hub.v1`**, from `hub/src/gen/`.
- **One version of a shared dependency.** A dependency more than one package declares is `catalog:` in each, pinned once in `pnpm-workspace.yaml`'s `catalog`. nama-ui takes React as a peer dependency, so an app and its components share one React.
- **Strictest lint.** The web app's `src/app/`, `src/features/` and `src/shared/`, nama-ui's `ui/src/`, and the Hub web app's `hub/src/app/` and `hub/src/features/` get extra oxlint rules (the `newFolders` override in `vite.config.ts`). The ones that change how code is written:
  - `satisfies` or a type guard, never a cast.
  - `""` and `0` aren't conditions: compare them. Checking whether an object is there is fine.
  - A function that returns a promise is `async`, and awaits rather than chaining `.then`.
  - At most 3 parameters, and JSX nested at most 2 deep, so a component that grows splits.
  - Named regex capture groups, explicit `public`/`private`, `globalThis`, no `reduce`, and no member access straight off an `await`.
  - Tests compare with `toStrictEqual` and keep setup inside `describe` or the hook functions.
- **Read `src/AGENTS.md` before you add or move a file under `src/`.** It holds the feature layout and the rules for its folders.
- **Read refusals by reason.** Choose a field or a reaction from the `Refusal` detail's `reason` and `field`, never from the message, which is only for showing.
- **Spans carry ids, not words.** A Diagnostics span's name and attributes hold only ids, counts, sizes, durations and outcomes, never a title, file path, name, IP address, search text or an error's message, since the Hub forwards them as they are (ADR 0054). Name a page by its route id, not its URL.
- **Glossary words in copy.** UI text uses `GLOSSARY.md` terms, in plain words.
- **No service worker.** Chrome installs the app without one, and one could keep serving an old app, or cached API answers, after the Server upgrades (ADR 0004).
