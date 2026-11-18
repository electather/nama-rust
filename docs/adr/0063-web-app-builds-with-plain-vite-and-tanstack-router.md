# The web app builds with plain Vite and TanStack Router, not TanStack Start

Supersedes ADR 0004's choice of TanStack Start; the rest of ADR 0004 stands.

The web app built with TanStack Start in SPA mode but used nothing from it at runtime: every routing piece (file routes, `beforeLoad`, loaders, code splitting) comes from TanStack Router. Start's real features are server-side rendering and server functions, and with a Rust Server there's no Node process to run them (ADR 0004). All Start did was prerender `_shell.html` at build time, which ran the root route's `beforeLoad` once, at build time, so an access check there never ran per visit. That trap needed its own rule, and the prerender's preview server needed a workaround where `localhost` resolves to IPv6.

So the app builds with plain Vite, React and TanStack Router's Vite plugin. `web/index.html` is the document, `src/main.tsx` renders the router into it, and the Server embeds the built `index.html` where it embedded `_shell.html`.

## Considered Options

- **Keep Start for SSR or server functions later**: rejected. Both need a Node process at runtime, which ADR 0004 rules out.

## Consequences

- The root route's `beforeLoad` runs on every visit. The access checks stay in each route's own `beforeLoad`, where they already were.
- `vp build` writes to `web/dist`, not `web/dist/client`.
