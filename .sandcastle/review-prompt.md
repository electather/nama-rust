# TASK

You are running AFK inside the sandcastle factory. Nobody can answer questions. Review and fix branch `{{BRANCH}}`, which implements issue #{{ISSUE_NUMBER}}: {{ISSUE_TITLE}}.

Commits under review:

!`git log origin/main..HEAD --oneline`

# REVIEW

Use the `code-review` skill with fixed point `origin/main`. The spec is issue #{{ISSUE_NUMBER}} (`gh issue view {{ISSUE_NUMBER}}`). The standards are `AGENTS.md`, `GLOSSARY.md`, `docs/adr/` and idiomatic Rust (clippy clean, no `unwrap()` outside tests, no needless `unsafe`).

If the diff touches async code (`async fn`, `tokio::`, `.await`, spawned tasks, channels, locks), also run the `tokio-async-code-review` skill over those files, and check its findings with `review-verification-protocol` as that skill says. Treat its Critical and Major findings as hard findings.

# FIX

Fix every hard finding and every Spec gap yourself, on this branch. Fix judgement-call smells only when the fix is clearly better. Don't add scope the issue didn't ask for.

Then run:

- `cargo fmt --all --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`

Commit the fixes (`refactor:` / `fix:` referencing `#{{ISSUE_NUMBER}}`). Do not push.

If the branch is fundamentally wrong and can't be fixed here, output `<blocked>one-line reason</blocked>`.

# PR BODY

Once the branch meets the spec and every check passes, write the pull request body for the branch as it now stands (`git diff origin/main...HEAD`), following this skill:

<skill name="pr">
!`awk 'f>=2{print} /^---$/{f++}' ~/.claude/skills/pr/SKILL.md`
</skill>

Use the real `cargo test` output you just ran as the Evidence. Output the body wrapped in a `pr-body` XML tag (opening and closing). Leave out `Closes #{{ISSUE_NUMBER}}`: the factory adds it.

Then output <promise>COMPLETE</promise>.
