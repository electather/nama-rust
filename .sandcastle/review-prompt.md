# TASK

You are running AFK inside the sandcastle factory. Nobody can answer questions. Review and fix branch `{{BRANCH}}`, which implements issue #{{ISSUE_NUMBER}}: {{ISSUE_TITLE}}.

Commits under review:

!`git log origin/main..HEAD --oneline`

# REVIEW

Use the `code-review` skill with fixed point `origin/main`. The spec is issue #{{ISSUE_NUMBER}} (`gh issue view {{ISSUE_NUMBER}}`). Its parent issue is {{PARENT}}: if there is one, use it as context for the Decisions, but don't count the parent's criteria that belong to sibling sub-issues as Spec gaps. The standards are `AGENTS.md`, `GLOSSARY.md`, `docs/adr/`, and the Review section of the area skill for each part of the repo the diff touches: `rust-crates` for Rust, `web-app` for `web/`.

# FIX

Fix every hard finding and every Spec gap yourself, on this branch. Fix judgement-call smells only when the fix is clearly better. Don't add scope the issue didn't ask for.

Findings in code this branch doesn't touch are parked (below).

Then run the Check section of each area skill the diff touches, and `web-app`'s Look section if the fixes changed a screen.

Commit the fixes (`refactor:` / `fix:` referencing `#{{ISSUE_NUMBER}}`). Do not push.

If the branch is fundamentally wrong and can't be fixed here, output `<blocked>one-line reason</blocked>`.

# PR BODY

Once the branch meets the spec and every check passes, write the pull request body for the branch as it now stands (`git diff origin/main...HEAD`), following this skill:

<skill name="pr">
!`awk 'f>=2{print} /^---$/{f++}' .claude/skills/pr/SKILL.md`
</skill>

Use the real output of the checks you just ran as the Evidence. Output the body wrapped in a `pr-body` XML tag (opening and closing). Leave out `Closes #{{ISSUE_NUMBER}}` and any `Part of` line: the factory adds them.

Then output <promise>COMPLETE</promise>.

# PARK

<skill name="park">
!`cat .sandcastle/park.md`
</skill>
