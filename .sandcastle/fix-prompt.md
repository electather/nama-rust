# TASK

Branch `{{BRANCH}}` (issue #{{ISSUE_NUMBER}}: {{ISSUE_TITLE}}) can't merge yet:

{{PROBLEM}}

# DO

- **Behind `origin/main` / conflicts**: run `git merge origin/main`. Resolve every conflict by reading both sides and keeping the intent of both. Don't drop work from main.
- **CI failing**: read the failures with `gh pr checks {{BRANCH}}`, then `gh run view <run-id> --log-failed`. Fix the root cause, not the test.

Then run `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace`, and commit. Do not push.

If you can't fix it, output `<blocked>one-line reason</blocked>`. Otherwise output <promise>COMPLETE</promise>.
