# The web app is organised by feature, with rules per folder

The web app grew in three flat folders: `components/`, `lib/` and `routes/`, with every test beside them in `src/`. A change to one screen touched all three, and nothing said which components belonged to which screen, so code one screen used and code every screen used looked the same. The app is now organised by feature: `src/features/<feature>/` holds a feature's components, data hooks, other hooks, helpers and tests, `src/shared/` holds what more than one feature uses, `src/app/` holds the shell, and route files stay thin. `web/src/AGENTS.md` has the layout and its rules.

The rules sit with the folders they govern. Each folder's `AGENTS.md` holds its own rules, Fallow's zones say which folder imports which, and the strictest lint rules apply through an oxlint override on the new folders. An agent working in a folder reads only the rules that apply there.

The strict rules apply to a folder as code moves into it, not to all code at once. Rules that fix themselves, or had no hits, were turned on everywhere in one mechanical change. The rest (one component per file, no casts, no `""` or `0` as a condition, data only through data hooks and Suspense) need a person to fix each hit, and the old code had hundreds. Each move PR brings one feature up to them, and the legacy folders keep their old rules until the last move (#1150) deletes them.

## Considered Options

- **A `components/`, `layouts/`, `utils/` split by kind**: rejected. It's the flat layout again with more folders: a screen's code still spreads across all of them, and nothing marks what's shared.
- **One set of rules for the whole app, turned on at once**: rejected. Fixing hundreds of hits by hand in one change buries every real change in churn, and every move PR would conflict with it.
- **Warnings for legacy code**: rejected. Every rule is `error`, so a finding never becomes noise everyone learns to ignore.

## Consequences

- Each move PR adds the feature's Fallow zone, listing the sibling features it may import, so coupling between features is explicit.
- Today's files over 200 lines are listed by name in `web/.fallowrc.jsonc`, and the list shrinks as files move.
- Until #1150, the new zones may import the legacy ones.
