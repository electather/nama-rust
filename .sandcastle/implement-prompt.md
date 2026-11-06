Implement ticket #{{ISSUE_NUMBER}} ({{ISSUE_TITLE}}) by following these skill instructions:

<skill name="implement">
!`awk 'f>=2{print} /^---$/{f++}' ~/.claude/skills/implement/SKILL.md`
</skill>

Write the code under these three skills as well:

<skill name="rust-best-practices">
!`awk 'f>=2{print} /^---$/{f++}' ~/.claude/skills/rust-best-practices/SKILL.md`
</skill>

Its `references/` chapters live in `~/.claude/skills/rust-best-practices/references/`. Read a chapter only when the ticket touches that topic.

<skill name="ponytail">
!`awk 'f>=2{print} /^---$/{f++}' "$(ls ~/.claude/plugins/cache/ponytail/ponytail/*/skills/ponytail/SKILL.md | tail -1)"`
</skill>

<skill name="clean-code">
!`awk 'f>=2{print} /^---$/{f++}' ~/.claude/skills/clean-code/SKILL.md`
</skill>

If the ticket touches the web UI (React, TypeScript or CSS for the browser, ADR 0004), also follow the `impeccable` skill: read `~/.claude/skills/impeccable/SKILL.md` and the references it points to, run its `context` step from the repo root, and build to `PRODUCT.md` and `DESIGN.md`. Where it says to ask the user, decide yourself, guided by those two files. There is no browser in the sandbox, so where it says to look at the rendered result, run `~/.claude/skills/impeccable/scripts/impeccable detect --json <changed UI files>` instead and fix what it finds. Skip all of this for Rust-only tickets.

Ponytail decides how much to build: no abstraction, file or layer the ticket doesn't need. Rust best practices and clean code decide how it reads: idioms, names, function size, comments that say why. Where they pull apart, ponytail wins, and `AGENTS.md` beats all three.

You are running AFK inside the sandcastle factory, on branch `{{BRANCH}}` forked from `origin/main`. Nobody can answer questions. Where the skill says "ask", decide yourself and record the decision in the commit body, or give up with `<blocked>`.

Factory rules on top of the skill:

- Read `AGENTS.md` first. Use `GLOSSARY.md` vocabulary and respect `docs/adr/`.
- Parent issue: {{PARENT}}. If there is one, read it (`gh issue view`) for the overall picture and its Decisions, but build only this ticket's acceptance criteria. Its sibling sub-issues own the rest of the parent's scope; don't do their work.
- "Typechecking" and "tests" for this Rust repo mean: `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, plus anything the issue's acceptance criteria name (for example `buf lint`).
- Use conventional commit messages that reference `#{{ISSUE_NUMBER}}`. Do not push, open a PR, or close or relabel the issue: the factory does that.
- If you can't finish (missing info, the spec contradicts an ADR, you're blocked outside the repo), commit what is solid and output `<blocked>one-line reason</blocked>`.
- When every acceptance criterion is met and the checks pass, output <promise>COMPLETE</promise>.
