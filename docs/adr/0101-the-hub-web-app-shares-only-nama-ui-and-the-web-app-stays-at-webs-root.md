# The Hub web app shares only nama-ui, and the web app stays at `web/`'s root

_Superseded by [ADR 0105](0105-the-hub-serves-the-web-app-for-all-of-an-accounts-servers.md)._

The Hub's web pages (ADR 0010) are a second app, and they should look like the Server's web app. So `web/` is a pnpm workspace, and the two apps share one package, nama-ui (`web/ui/`): the shadcn components with their tokens and fonts (ADR 0016), and nothing else. The web app's shared components, hooks, `src/shared/lib/` and generated client stay in it. They're built on the Server's API (`nama.v1`), its Refusals and its pages, while the Hub speaks its own API and has pages of its own. Sharing them would put the Server's client in the Hub's bundle, and any change to one would change two apps. A design-system component the Hub comes to need moves into nama-ui once it does. A component that calls an RPC never does.

nama-ui ships its sources, with no build step. Each app's Vite and Tailwind compile them, so there's nothing to rebuild before a change shows, and `vp check` and Fallow check it from the web app's config, with none of its own.

The web app stays at `web/`'s root, with nama-ui in `web/ui/` and the Hub web app joining as `web/hub/`. It doesn't move to `apps/web/` beside `packages/ui/`. vite-plus 1.6.0 builds the root's own app when the workspace root has an `index.html` and a `vite.config.ts`, even with another app in the workspace, so the root can be both. Moving it would change every path that names it (the Server's embed of `web/dist`, `buf generate`'s output in `web/src/gen`, CI's working directories, the flaky-test history keyed by each test's file, the impeccable config and every `AGENTS.md` that names `web/src/`) and the history of over a thousand files, for a layout that reads a little tidier.

## Considered Options

- **`apps/` and `packages/`**: rejected, for the moves above.
- **The Hub web app importing the web app's `src/` by path**: rejected. With no package between them, nothing stops the Hub from reaching past the design system into the Server's components.
- **Sharing `src/shared/` whole**: rejected, for the Server's client it brings.
- **Building nama-ui into `dist/`**: rejected. A build step and its output to keep current, for two private apps in one repo.

## Consequences

- ADR 0016's tokens live in `web/ui/src/styles.css` now. The web app's `src/styles.css` imports them and adds its own tokens and utilities. Each package's formatter and lint read its own stylesheet, so a nama-ui component that uses a class only the web app defines fails `vp check`.
- A dependency both packages declare is pinned once in the pnpm `catalog`, and React is nama-ui's peer dependency, so an app and its components share one React.
- nama-ui's package `exports` make each component a Fallow entry point, so Fallow checks its exports (`includeEntryExports`) to keep failing a component no screen uses.
