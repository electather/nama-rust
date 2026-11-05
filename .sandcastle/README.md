# AI factory (sandcastle)

Takes open `ready-for-agent` issues with no open blockers, up to 3 at a time, through:
`/implement` → `code-review` + fixes → PR (`Closes #N`) → CI (one auto-fix attempt) → squash-merge.
Anything stuck is relabelled `ready-for-human` with a comment.

One-time setup (from the repo root):

    npm --prefix .sandcastle install
    npx --prefix .sandcastle sandcastle docker build-image   # rebuild after editing Dockerfile
    cp .sandcastle/.env.example .sandcastle/.env              # CLAUDE_CODE_OAUTH_TOKEN (`claude setup-token`), GH_TOKEN (`gh auth token`)

Run (drains the queue, then exits):

    npm --prefix .sandcastle run factory

Logs: `.sandcastle/logs/`. Agents use the host skills mounted from `~/.agents/skills`.
