## Workspace rules

Read `README.md` for the layout and the checks CI runs. Rules for one package live in that package's own `AGENTS.md` (`crates/`, `crates/nama-server/`, `crates/nama-proto/`, `plugins/`, `web/`). Read it before you change anything there. This file holds only rules for the whole workspace.

- **Lints.** CI denies every warning. To silence a lint, use `#[expect(lint, reason = "...")]` on the smallest item possible, never a bare `#[allow]`.
- **Docs.** Every `pub` or exported item gets a one-line doc comment saying what it's for (`missing_docs`); it grows only for what the signature can't show, such as errors, ordering or who may call it. A private item gets one only when its name and signature leave something out.
- **Comments say why:** a reason, an outside constraint (SQLite, a protocol, an ADR), a trap for the next editor, or a `ponytail:` ceiling. What the code says, a clearer name, a type or an assertion says instead. A test's name says what it checks.
- **Rules live in `AGENTS.md`.** A rule future code in a package must follow goes once in that package's `AGENTS.md`; code that enforces it points there in one line. A package's `README.md` says how to work on it.

## Agent skills

### Issue tracker

Issues are tracked in GitHub Issues for electather/nama-rust (via `gh`). See `docs/agents/issue-tracker.md`.

### Triage labels

Default vocabulary: `needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human`, `wontfix`. See `docs/agents/triage-labels.md`.

### Domain docs

Single-context: one `GLOSSARY.md` + `docs/adr/` at the repo root. See `docs/agents/domain.md`.
