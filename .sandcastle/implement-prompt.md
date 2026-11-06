Implement ticket #{{ISSUE_NUMBER}} ({{ISSUE_TITLE}}) by following these skill instructions:

<skill name="implement">
!`awk 'f>=2{print} /^---$/{f++}' ~/.claude/skills/implement/SKILL.md`
</skill>

Write the code under these two skills as well:

<skill name="ponytail">
!`awk 'f>=2{print} /^---$/{f++}' "$(ls ~/.claude/plugins/cache/ponytail/ponytail/*/skills/ponytail/SKILL.md | tail -1)"`
</skill>

<skill name="clean-code">
!`awk 'f>=2{print} /^---$/{f++}' ~/.claude/skills/clean-code/SKILL.md`
</skill>

Before writing code, load the area skill for each part of the repo the ticket touches: `rust-crates` for Rust, `web-app` for `web/`. Build to it, and run its Check section (and `web-app`'s Look section) before each commit.

Ponytail decides how much to build: no abstraction, file or layer the ticket doesn't need. The area skills and clean code decide how it reads: idioms, names, function size, comments that say why. Where they pull apart, ponytail wins, and `AGENTS.md` beats them all.

You are running AFK inside the sandcastle factory, on branch `{{BRANCH}}` forked from `origin/main`. Nobody can answer questions. Where the skill says "ask", decide yourself and record the decision in the commit body, or give up with `<blocked>`.

Factory rules on top of the skill:

- Read `AGENTS.md` first. Use `GLOSSARY.md` vocabulary and respect `docs/adr/`.
- Parent issue: {{PARENT}}. If there is one, read it (`gh issue view`) for the overall picture and its Decisions, but build only this ticket's acceptance criteria. Its sibling sub-issues own the rest of the parent's scope; don't do their work.
- "Typechecking" and "tests" mean the Check section of each area skill the ticket touches, plus anything the issue's acceptance criteria name (for example `buf lint`).
- Use conventional commit messages that reference `#{{ISSUE_NUMBER}}`. Do not push, open a PR, or close or relabel the issue: the factory does that.
- If you can't finish (missing info, the spec contradicts an ADR, you're blocked outside the repo), commit what is solid and output `<blocked>one-line reason</blocked>`.
- When every acceptance criterion is met and the checks pass, output <promise>COMPLETE</promise>.
