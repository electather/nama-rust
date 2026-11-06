## Workspace rules

Read `README.md` for the layout and the checks CI runs. Rules for one package live in that package's own `AGENTS.md` (`crates/`, `crates/nama-server/`, `crates/nama-proto/`, `plugins/`, `web/`). Read it before you change anything there.

- **Lints.** CI denies every warning. To silence a lint, use `#[expect(lint, reason = "...")]` on the smallest item possible, never a bare `#[allow]`.
- **Docs.** Every `pub` item gets a doc comment saying what it's for (`missing_docs`). Rustdoc says what code is and why. A package's `README.md` says how to work on it. A package's `AGENTS.md` holds the rules for that package. This file holds only rules for the whole workspace.

## Agent skills

### Issue tracker

Issues are tracked in GitHub Issues for electather/nama-rust (via `gh`). See `docs/agents/issue-tracker.md`.

### Triage labels

Default vocabulary: `needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human`, `wontfix`. See `docs/agents/triage-labels.md`.

### Domain docs

Single-context: one `GLOSSARY.md` + `docs/adr/` at the repo root. See `docs/agents/domain.md`.
