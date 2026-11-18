## web

- **Load the `web-app` skill** before you change anything here. It holds building to `PRODUCT.md` and `DESIGN.md` with impeccable, the checks, the screenshot step and the review.
- **Never edit `src/gen/` or `src/routeTree.gen.ts` by hand.** Change `proto/` and run `buf generate` there, or let the router plugin regenerate the route tree.
- **Only `vp`.** Run, build, check and test through `vp` (ADR 0017). Don't add ESLint, Prettier, Biome, a separate `vitest.config.ts` or `.oxlintrc`. Config goes in `vite.config.ts`. An ESLint plugin oxlint loads as a JS plugin is fine: it runs inside `vp check`.
- **Suppressions name a reason.** To silence a lint, use `// oxlint-disable-next-line <rule> -- <reason>` on the line, never a whole file. For a type error, use `// @ts-expect-error <reason>`, never `@ts-ignore`. To raise a complexity limit, use a Fallow `thresholdOverrides` entry with a `reason`.
- **Semantic tokens only.** Style with shadcn's semantic utilities (`bg-background`, `text-muted-foreground`, `bg-primary`). Never use Tailwind palette colours, raw colour values or `dark:` variants (ADR 0016). If a screen needs a colour that no token covers, add a token to `src/styles.css`.
- **Logical properties.** Use `ms-`/`me-`/`ps-`/`pe-`/`start-`/`end-`, never `ml-`/`mr-`/`pl-`/`pr-`/`left-`/`right-`, so right-to-left languages can be added later.
- **Imports go one way.** `src/gen/` and the shadcn components import nothing from the app, and only the router imports a route file. `src/AGENTS.md`, "Layers", has the rest. Fallow enforces it.
- **Read `src/AGENTS.md` before you add or move a file under `src/`.** It holds the feature layout and the rules for its folders.
- **Read refusals by reason.** Choose a field or a reaction from the `Refusal` detail's `reason` and `field`, never from the message, which is only for showing.
- **Sensitive changes ask "Confirm it’s you".** Call an RPC that needs Recent Authentication (`crates/nama-server/AGENTS.md`) with `useSensitiveMutation`, never connect-query's `useMutation`. `web/README.md` says how it works.
- **Spans carry ids, not words.** A Diagnostics span's name and attributes hold only ids, counts, sizes, durations and outcomes, never a title, file path, name, IP address, search text or an error's message, since the Hub forwards them as they are (ADR 0054). Name a page by its route id, not its URL.
- **Glossary words in copy.** UI text uses `GLOSSARY.md` terms, in plain words. A User's own page is "Settings" (`/settings`), never "Account": an Account is the Hub identity. Don't use `/link` for anything but Linking an Account.
- **No service worker.** Chrome installs the app without one, and one could keep serving an old app, or cached API answers, after the Server upgrades (ADR 0004).
