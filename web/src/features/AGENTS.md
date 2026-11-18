## web/src/features

One folder per feature, named in kebab case for what a person does there (`requests`, `admin-users`, `player`). Every feature has the same anatomy, and only the folders it needs:

```
features/<feature>/
  components/   its components, one per file; the page a route renders is one of them
  data/         its data hooks, one per file (`use-requests.ts`): the only files here that
                import connect-query or TanStack Query
  hooks/        its other hooks, one per file
  lib/          its functions, types and constants, with no React
  __tests__/    its tests, which test what a person sees on its pages
```

- **A feature's zone.** The PR that adds a feature adds its zone to `web/.fallowrc.jsonc`, allowed the `shared-*` zones and the sibling features it lists, and adds it to the `app`, `routes` and `router` zones that render it. A sibling a feature imports is an explicit choice, made in that list.
- **Only the route renders the page.** A route file in `src/routes/` imports the feature's page component and passes it what the loader and search params give. The feature knows nothing of the route that renders it.
- **Shared when a second feature needs it.** Code one feature uses stays in that feature. When another feature needs it, it moves to `src/shared/`, in that PR.
