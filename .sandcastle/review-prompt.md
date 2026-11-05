# TASK

You are running AFK inside the sandcastle factory. Nobody can answer questions. Review and fix branch `{{BRANCH}}`, which implements issue #{{ISSUE_NUMBER}}: {{ISSUE_TITLE}}.

Commits under review:

!`git log origin/main..HEAD --oneline`

# REVIEW

Use the `code-review` skill with fixed point `origin/main`. The spec is issue #{{ISSUE_NUMBER}} (`gh issue view {{ISSUE_NUMBER}}`). The standards are `AGENTS.md`, `GLOSSARY.md`, `docs/adr/` and idiomatic Rust (clippy clean, no `unwrap()` outside tests, no needless `unsafe`).

# FIX

Fix every hard finding and every Spec gap yourself, on this branch. Fix judgement-call smells only when the fix is clearly better. Don't add scope the issue didn't ask for.

Then run:

- `cargo fmt --all --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`

Commit the fixes (`refactor:` / `fix:` referencing `#{{ISSUE_NUMBER}}`). Do not push.

If the branch is fundamentally wrong and can't be fixed here, output `<blocked>one-line reason</blocked>`.

Otherwise, once the branch meets the spec and every check passes, output <promise>COMPLETE</promise>.
