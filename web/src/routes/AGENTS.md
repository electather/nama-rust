## web/src/routes

TanStack Router's file routes.

- **Route files are thin.** A route file holds the route's options (`validateSearch`, `beforeLoad`, `loader`, `pendingComponent`, `errorComponent`, `notFoundComponent`, `staticData`) and a component that reads the route's params, search and context and renders one page or layout from `src/app/` or a feature, passing them as props. Any other component, hook or helper goes there.
- **Every code file here is a route.** The router plugin ignores nothing but generated files. Don't keep a component, hook, helper or test here behind its `-` prefix or a `routeFileIgnorePattern`: it goes in `src/app/` or a feature.
- **Access checks per route.** Put a signed-in page under `_member/`, and an Admin page under `_member/_admin/`. The checks run in the `beforeLoad` of `/setup`, `/sign-in` and `_member` (and `_admin` for its Permission), through `src/shared/hooks/access.ts`, not in the root's.
