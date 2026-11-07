# AI factory (sandcastle)

Takes open `ready-for-agent` issues with no open blockers, up to 3 at once (`MAX_PARALLEL`), each through:
`implement` skill → `code-review` skill + fixes → PR (`Closes #N`) → CI (one auto-fix attempt) → squash-merge.
While an issue is being built it carries `agent-working`; the label comes off when it merges, is handed back or the run crashes.
Only one factory runs per machine (`~/.cache/nama-sandcastle/factory.pid`). Factories on different machines can run at once: each claims an issue by creating `refs/claims/issue-N` on origin, which only one of them can do, and deletes it when done or on Ctrl-C. A run that was killed outright leaves its claims behind. Free one with `git push origin --delete refs/claims/issue-N` and remove the issue's `agent-working` label.
Anything stuck is relabelled `ready-for-human` with a comment.

It builds web tickets as well as Rust ones. The image (`Dockerfile`, on `node:26-bookworm`) holds the Rust toolchain with the `wasm32` target, `buf`, `gh`, Claude Code, Vite+ (`vp`) with the Node 26 and pnpm 12 that `web/` pins, and Playwright's headless Chromium. On a ticket that touches `web/`, agents run the web job's checks and screenshot the screens they changed at phone and desktop widths to check them against `DESIGN.md`. The cargo registry and the pnpm store are cached on the host in `~/.cache/nama-sandcastle/`, so each sandbox doesn't download them again.

An issue with sub-issues is never built directly. The factory builds its sub-issues (each gets the parent as context) and closes the parent once the last one merges.

Agents are Claude Code (`claude-opus-5-5`, effort `high`) in Docker. Your host skills come from `~/.agents/skills`, mounted read-only. The implement step also inlines `clean-code` and the `ponytail` plugin skill, and the review step writes the PR body with `pr`. What to write, check and review for each part of the repo lives in the repo's area skills, `.claude/skills/rust-crates` and `.claude/skills/web-app`, which every step loads for the parts a ticket touches. They pull in `rust-best-practices`, `tokio-async-code-review` and `impeccable`. Install the non-plugin ones with:

    npx skills@latest add mattpocock/skills --skill=pr -g

    npx skills add apollographql/skills --skill rust-best-practices -g
    npx skills add https://github.com/sickn33/agentic-awesome-skills --skill clean-code -g
    npx skills add existential-birds/beagle --skill tokio-async-code-review --skill review-verification-protocol -g
    npx skills add pbakaus/impeccable -g   # UI tickets only: implement and review follow it against PRODUCT.md / DESIGN.md

The `impeccable` launcher downloads its Linux engine into the container's `~/.impeccable` on first use, so a UI ticket needs network access the first time each sandbox runs it. Its post-edit hook isn't installed in the container. The `web-app` skill's checks run `impeccable detect` instead.

One-time setup (from the repo root):

    npm --prefix .sandcastle install
    npx --prefix .sandcastle sandcastle docker build-image   # rebuild after editing Dockerfile
    cp .sandcastle/.env.example .sandcastle/.env              # CLAUDE_CODE_OAUTH_TOKEN (`claude setup-token`), GH_TOKEN (`gh auth token`)

Run from the repo root:

    npm --prefix .sandcastle run factory                  # drain the queue (at most 60 issues), then exit
    MAX_ISSUES=1 npm --prefix .sandcastle run factory     # one issue, then stop

Logs: `.sandcastle/logs/`.

Stopping gracefully: `touch .sandcastle/STOP`. Issues already running finish (merged or handed back), nothing new starts, and the run exits. Later runs, such as a scheduled one, exit at once while the file exists. `rm .sandcastle/STOP` to resume.

Stopping and resuming: Ctrl-C keeps branches, commits and worktrees. The next run continues on the same `agent/issue-N` branch and PR. Remove leftover containers with `docker ps --filter ancestor=sandcastle:nama-rust`.

Failures:

- **Agent gives up or CI stays red:** the issue is relabelled `ready-for-human`.
- **Infra error (Docker, auth, Claude usage limit):** nothing is relabelled. The run finishes shipping what is already built, then exits with code 1. Rerun once the problem is fixed.
