## web

- **Load the `web-app` skill** before you change anything here. It holds building to `PRODUCT.md` and `DESIGN.md` with impeccable, the checks, the screenshot step and the review.
- **Never edit `src/gen/` or `src/routeTree.gen.ts` by hand.** Change `proto/` and run `buf generate` there, or let the router plugin regenerate the route tree.
- **Only `vp`.** Run, build, check and test through `vp` (ADR 0017). Don't add ESLint, Prettier, Biome, a separate `vitest.config.ts` or `.oxlintrc`. Config goes in `vite.config.ts`.
- **Suppressions name a reason.** To silence a lint, use `// oxlint-disable-next-line <rule> -- <reason>` on the line, never a whole file. For a type error, use `// @ts-expect-error <reason>`, never `@ts-ignore`. To raise a complexity limit, use a Fallow `thresholdOverrides` entry with a `reason`.
- **Semantic tokens only.** Style with shadcn's semantic utilities (`bg-background`, `text-muted-foreground`, `bg-primary`). Never use Tailwind palette colours, raw colour values or `dark:` variants (ADR 0016). If a screen needs a colour that no token covers, add a token to `src/styles.css`.
- **Logical properties.** Use `ms-`/`me-`/`ps-`/`pe-`/`start-`/`end-`, never `ml-`/`mr-`/`pl-`/`pr-`/`left-`/`right-`, so right-to-left languages can be added later.
- **Owned components.** `src/components/ui/` is our code. Restyle what shadcn generates to `DESIGN.md`, and add a component only when a screen uses it. Re-running `shadcn add` overwrites the restyle, so diff it.
- **Imports go one way.** `src/gen/` and `src/components/ui/` import nothing from the app. `src/components/` and `src/lib/` never import from `src/routes/`. Nothing imports a route file. Fallow enforces this.
- **Glossary words in copy.** UI text uses `GLOSSARY.md` terms, in plain words. A User's own page is "Settings" (`/settings`), never "Account": an Account is the Hub identity. Don't use `/link` for anything but Linking an Account.
