## web/src/routes

TanStack Router's file routes.

- **Route files are thin.** A route file holds the route's options (`validateSearch`, `beforeLoad`, `loader`, `pendingComponent`, `errorComponent`, `notFoundComponent`, `staticData`) and a component that reads the route's params, search and context and renders one page or layout from `src/app/` or a feature, passing them as props. Any other component, hook or helper goes there.
- **Every code file here is a route.** The router plugin ignores nothing but generated files. Don't keep a component, hook, helper or test here behind its `-` prefix or a `routeFileIgnorePattern`: it goes in `src/app/` or a feature.
- **Access checks per route.** Put a signed-in page under `_member/`. An admin view is a section of Settings, not a page. The checks run in the `beforeLoad` of `/setup`, `/sign-in` and `_member`, through `src/shared/hooks/access.ts`, not in the root's.
- **Each step is a page.** A step of a flow, such as getting a Sign-in Code and then waiting on it, is a route of its own, so back and forward step through it; never a component swapped by state. Pages that share a frame share a layout route (`_signed-out`). What one step hands the next that mustn't go in the address rides in history state, typed in `src/features/auth/lib/history-state.ts`. A step that can't be redone, such as an approved code, gives way with `replace`.
