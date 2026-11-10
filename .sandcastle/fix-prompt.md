# TASK

Branch `{{BRANCH}}` (issue #{{ISSUE_NUMBER}}: {{ISSUE_TITLE}}, parent issue: {{PARENT}}) can't merge yet:

{{PROBLEM}}

# DO

- **Behind `origin/main` / conflicts**: run `git merge origin/main`. Resolve every conflict by reading both sides and keeping the intent of both. Don't drop work from main.
- **CI failing**: read the failures with `gh pr checks {{BRANCH}}`, then `gh run view <run-id> --log-failed`. Fix the root cause, not the test.

The merge or the CI failure is your whole diff; park anything else you find (below).

Then run the Check section of the area skill for each part of the repo the branch touches (`rust-crates` for Rust, `web-app` for `web/`), and commit. Do not push.

If you can't fix it, output `<blocked>one-line reason</blocked>`. Otherwise output <promise>COMPLETE</promise>.

<skill name="park">
!`cat .sandcastle/park.md`
</skill>
