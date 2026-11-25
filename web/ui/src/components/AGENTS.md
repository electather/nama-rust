## web/ui/src/components

nama-ui's shadcn components, which the web app imports as `nama-ui/components/<file>`. `web/AGENTS.md` has the rules every web package shares.

- **Owned components.** These shadcn components are our code. Restyle what shadcn generates to `DESIGN.md`, and add a component only when a screen uses it. Re-running `shadcn add` overwrites the restyle, so diff it.
- **shadcn's file shapes.** A file keeps a component's sub-parts and variants together, as shadcn writes it, so the web app's one-component-per-file rule doesn't apply here. Otherwise the strictest lint applies (`web/AGENTS.md`, "Strictest lint").
- **Every export has a user.** Fallow fails a component, or a sub-part, that no app imports, so delete what a `shadcn add` brings that no screen uses.
