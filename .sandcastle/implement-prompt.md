/implement #{{ISSUE_NUMBER}} ({{ISSUE_TITLE}})

You are running AFK inside the sandcastle factory, on branch `{{BRANCH}}` forked from `origin/main`. Nobody can answer questions. Where the skill says "ask", decide yourself and record the decision in the commit body, or give up with `<blocked>`.

Factory rules on top of the skill:

- Read `AGENTS.md` first. Use `GLOSSARY.md` vocabulary and respect `docs/adr/`.
- "Typechecking" and "tests" for this Rust repo mean: `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, plus anything the issue's acceptance criteria name (for example `buf lint`).
- Use conventional commit messages that reference `#{{ISSUE_NUMBER}}`. Do not push, open a PR, or close or relabel the issue: the factory does that.
- If you can't finish (missing info, the spec contradicts an ADR, you're blocked outside the repo), commit what is solid and output `<blocked>one-line reason</blocked>`.
- When every acceptance criterion is met and the checks pass, output <promise>COMPLETE</promise>.
