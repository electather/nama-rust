# AI factory (sandcastle)

Takes open `ready-for-agent` issues with no open blockers, one at a time, through:
`implement` skill → `code-review` skill + fixes → PR (`Closes #N`) → CI (one auto-fix attempt) → squash-merge.
Anything stuck is relabelled `ready-for-human` with a comment.

An issue with sub-issues is never built directly. The factory builds its sub-issues (each gets the parent as context) and closes the parent once the last one merges.

Agents are Claude Code (`claude-opus-5-5`, effort `high`) in Docker. Your host skills come from `~/.agents/skills`, mounted read-only. The implement step also inlines `rust-best-practices`, `clean-code` and the `ponytail` plugin skill; the review step adds `tokio-async-code-review` for async diffs and writes the PR body with `pr`. Install the non-plugin ones with:

    npx skills@latest add mattpocock/skills --skill=pr -g

    npx skills add apollographql/skills --skill rust-best-practices -g
    npx skills add https://github.com/sickn33/agentic-awesome-skills --skill clean-code -g
    npx skills add existential-birds/beagle --skill tokio-async-code-review --skill review-verification-protocol -g
    npx skills add pbakaus/impeccable -g   # UI tickets only: implement and review follow it against PRODUCT.md / DESIGN.md

The `impeccable` launcher downloads its Linux engine into the container's `~/.impeccable` on first use, so a UI ticket needs network access the first time each sandbox runs it. Its post-edit hook isn't installed in the container. The review step runs `impeccable detect` instead.

One-time setup (from the repo root):

    npm --prefix .sandcastle install
    npx --prefix .sandcastle sandcastle docker build-image   # rebuild after editing Dockerfile
    cp .sandcastle/.env.example .sandcastle/.env              # CLAUDE_CODE_OAUTH_TOKEN (`claude setup-token`), GH_TOKEN (`gh auth token`)

Run from the repo root:

    npm --prefix .sandcastle run factory                  # drain the queue (max 20 batches), then exit
    MAX_CYCLES=1 npm --prefix .sandcastle run factory     # one batch, then stop

Logs: `.sandcastle/logs/`.

Stopping and resuming: Ctrl-C keeps branches, commits and worktrees. The next run continues on the same `agent/issue-N` branch and PR. Remove leftover containers with `docker ps --filter ancestor=sandcastle:nama-rust`.

Failures:

- **Agent gives up or CI stays red:** the issue is relabelled `ready-for-human`.
- **Infra error (Docker, auth, Claude usage limit):** nothing is relabelled. The run finishes shipping what is already built, then exits with code 1. Rerun once the problem is fixed.
