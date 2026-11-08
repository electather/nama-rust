# AI factory (sandcastle)

Takes open `ready-for-agent` issues with no open blockers, up to 2 at once (`MAX_PARALLEL=N` to change), each through:
`implement` skill → `code-review` skill + fixes → PR (`Closes #N`) → CI (one auto-fix attempt) → squash-merge.
While an issue is being built it carries `agent-working`; the label comes off when it merges, is handed back or the run crashes.
Only one factory runs per machine (`~/.cache/nama-sandcastle/factory.pid`). Factories on different machines can run at once: each claims an issue by creating `refs/claims/issue-N` on origin, which only one of them can do, and deletes it when done or on Ctrl-C. A run that was killed outright leaves its claims behind. Free one with `git push origin --delete refs/claims/issue-N` and remove the issue's `agent-working` label.
Anything stuck is relabelled `ready-for-human` with a comment.

It builds web tickets as well as Rust ones. The image (`Dockerfile`, on `node:26-bookworm`) holds the Rust toolchain with the `wasm32` target (linking with `mold`, at most 4 cargo jobs, line-table debug info, so two sandboxes fit in OrbStack's memory), `buf`, `gh`, Claude Code, Vite+ (`vp`) with the Node 26 and pnpm 12 that `web/` pins, and Playwright's headless Chromium. On a ticket that touches `web/`, agents run the web job's checks and screenshot the screens they changed at phone and desktop widths to check them against `DESIGN.md`. The cargo registry and the pnpm store are cached on the host in `~/.cache/nama-sandcastle/`, so each sandbox doesn't download them again.

An issue with sub-issues is never built directly, and it doesn't carry `ready-for-agent` (see `docs/agents/triage-labels.md`). The factory builds its labelled sub-issues (each gets the parent as context) and closes the parent once the last one is closed.

Agents are Claude Code (`claude-opus-5-5`, effort `high`) in Docker. Their skills are the repo's own: third-party ones live in `.agents/skills`, linked from `.claude/skills` and pinned in `skills-lock.json`, so each worktree brings them from `origin/main`. A skill change reaches the factory only once it's merged. The implement step also inlines `clean-code` and `ponytail`, and the review step writes the PR body with `pr`. What to write, check and review for each part of the repo lives in the repo's area skills, `.claude/skills/rust-crates` and `.claude/skills/web-app`, which every step loads for the parts a ticket touches. They pull in `rust-best-practices`, `tokio-async-code-review` and `impeccable`.

`ponytail` is still a host skill, mounted read-only from `~/.agents/skills/ponytail`:

    npx skills add DietrichGebert/ponytail --skill ponytail -g

### TokenEconomizer (optional, per machine)

[TokenEconomizer](https://github.com/microsoft/TokenEconomizer) learns from each agent session in this repo (repo facts, unused tools, stuck turns) to make later sessions cheaper. The factory runs it in every sandbox when the clone is on the machine; without it, or with `TE=off`, the factory runs as before. Set it up on each factory machine:

    git clone https://github.com/microsoft/TokenEconomizer.git ~/Code/TokenEconomizer   # or anywhere, with TE_CLONE=<path>

That's all the factory needs: no image rebuild, no `claude plugin install` (that's only for your own sessions on the machine). The factory mounts three things into each sandbox: the clone (read-only, run from there, so `git pull` in it takes effect at the next sandbox), a state folder, and `claude-settings.json` as the container's `~/.claude/settings.json` (the plugin's hooks with the mounted path). The hooks need the base image's `python3`, so keep `node:26-bookworm`, not `-slim`.

Where it learns, each machine apart (nothing syncs between machines):

- **Native Linux** (Docker or Podman): `~/.tokeneconomizer`, the same state your own sessions on the machine use, so they learn together. Safe because host and containers share one kernel, so TokenEconomizer's `flock` excludes both.
- **macOS, or Docker Desktop / OrbStack anywhere**: `~/.cache/nama-sandcastle/tokeneconomizer`, the factory's own. Containers run in a VM there and can't see the host's `flock` over the file share, so a shared state would corrupt when a host session and a sandbox wrote at once. Sandboxes still exclude each other.

Check it after a run (on Linux drop the `TOKENECONOMIZER_HOME=` prefix; `sessions` should count the factory's runs):

    TOKENECONOMIZER_HOME=~/.cache/nama-sandcastle/tokeneconomizer ~/Code/TokenEconomizer/plugin/scripts/te --cwd . status

Settings live in that state folder, per repo, so on macOS the factory's are separate from yours. Micro-skills (learned facts) start off; turn them on with the same prefix and `config set microskills=on` (it calls a model every 3 sessions, charged to the run; `microskills=envfacts` learns without one).

Tool pruning writes its deny list to the worktree's `.claude/settings.local.json`, which `.gitignore` keeps out of commits.

Restore the project skills after a clone, or update them, from the repo root:

    npx skills experimental_install
    npx skills update -p

The `impeccable` launcher downloads its Linux engine into the container's `~/.impeccable` on first use, so a UI ticket needs network access the first time each sandbox runs it. Its post-edit hook isn't installed in the container. The `web-app` skill's checks run `impeccable detect` instead.

One-time setup (from the repo root):

    npm --prefix .sandcastle install
    npx --prefix .sandcastle sandcastle docker build-image   # rebuild after editing Dockerfile
    # rootless Podman (detected from `docker --version`; the factory then uses sandcastle's podman sandbox):
    # podman build -t sandcastle:nama-rust --build-arg AGENT_UID=$(id -u) --build-arg AGENT_GID=$(id -g) -f .sandcastle/Dockerfile .sandcastle
    cp .sandcastle/.env.example .sandcastle/.env              # CLAUDE_CODE_OAUTH_TOKEN (`claude setup-token`), GH_TOKEN (`gh auth token`)

Run from the repo root:

    npm --prefix .sandcastle run factory                  # drain the queue (at most 60 issues), then exit
    MAX_ISSUES=1 npm --prefix .sandcastle run factory     # one issue, then stop

Unattended, retrying hourly after an infra failure such as the Claude usage limit (one supervisor per machine, `flock`):

    systemd-run --user --unit nama-factory --setenv=MAX_PARALLEL=3 -p WorkingDirectory=$PWD \
      -p StandardOutput=append:$PWD/.sandcastle/logs/supervisor.log -p StandardError=inherit .sandcastle/supervise.sh

`systemctl --user status nama-factory` shows it; `touch .sandcastle/STOP` ends it after the running issues finish.

Logs, all in `.sandcastle/logs/`:

- `status.md`: what is in flight (stage, PR, CI run), what is waiting and why (blocked, a parent, claimed by another factory), and the last 20 finished issues. It's rewritten on every change and covers this machine's factory only.
- `events.jsonl`: one JSON event per line, the record `status.md` is rendered from (`status.ts`). It keeps the history across runs.
- `launchd.log`: one timestamped line per event, stage changes with timings, and a summary of why the run ended and what is still queued.
- `crash-issue-N.log`: the full error behind a one-line crash in `launchd.log`.
- `agent-issue-N-<step>--N.log`: what each agent did.
- `mem.log`: every 30s, any sandbox over `MEM_LOG_GIB` (default 4) GiB, with its 8 biggest processes. Read it when OrbStack runs out of memory.

`npm --prefix .sandcastle run check` checks the `status.md` renderer.

Claude usage: before picking each issue the factory reads the 5-hour usage from the rate-limit headers of a 1-token Haiku call. At 75% or more (`USAGE_PAUSE=0.8` to change) it starts nothing new until the window resets, while running issues carry on. `launchd.log` says when it pauses (⏸) and resumes (▶).

Stopping gracefully: `touch .sandcastle/STOP`. Issues already running finish (merged or handed back), nothing new starts, and the run exits. Later runs, such as a scheduled one, exit at once while the file exists. `rm .sandcastle/STOP` to resume.

Stopping and resuming: Ctrl-C keeps branches, commits and worktrees. The next run continues on the same `agent/issue-N` branch and PR. Remove leftover containers with `docker ps --filter ancestor=sandcastle:nama-rust`.

Failures:

- **Agent gives up or CI stays red:** the issue is relabelled `ready-for-human`.
- **Issue closed while it's being built:** the factory notices within a minute (and at every stage change), stops that issue's agent, closes its PR and records it as cancelled. The rest of the run carries on. Once the merge has started, closing the issue no longer stops it.
- **PR merged by hand:** the factory notices within a minute, at any stage, stops that issue and records it as merged ("merged by hand"), closing finished parents as usual.
- **Infra error (Docker, auth, Claude usage limit):** nothing is relabelled. The run finishes shipping what is already built, then exits with code 1. Rerun once the problem is fixed.
