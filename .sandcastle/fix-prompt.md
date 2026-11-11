# TASK

Branch `{{BRANCH}}` (issue #{{ISSUE_NUMBER}}: {{ISSUE_TITLE}}, parent issue: {{PARENT}}) can't merge yet:

{{PROBLEM}}

# DO

- **Behind `origin/main` / conflicts**: run `git merge origin/main`. Resolve every conflict by reading both sides and keeping the intent of both. Don't drop work from main.
- **CI failing**: first run `git merge origin/main`. CI tests the branch merged into main, so a failure often comes from main moving since the branch started: a migration number or a list of migrations, a plan's reasons shifting by one, a type or function another PR changed. Then read the failures with `gh pr checks {{BRANCH}}`, then `gh run view <run-id> --log-failed`. Fix the root cause. When main's change made a test's expectation stale, updating the expectation is the fix. When this branch's change broke a test, fix the code, not the test.
- **The same failure is on main** (`gh run list --branch main --workflow CI --limit 1`, then its log): main is broken, not this branch. Don't fix main here, since every other open PR would fix it too and they'd conflict. Output `<blocked>main is red: one-line cause</blocked>`.

The merge or the CI failure is your whole diff; park anything else you find (below).

Then run the Check section of the area skill for each part of the repo the branch touches (`rust-crates` for Rust, `web-app` for `web/`), and commit. Do not push.

If you can't fix it, output `<blocked>one-line reason</blocked>`. Otherwise output <promise>COMPLETE</promise>.

<skill name="park">
!`cat .sandcastle/park.md`
</skill>
