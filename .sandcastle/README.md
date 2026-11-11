# AI factory (sandcastle)

One process runs three loops:

- **Triage:** open `needs-triage` issues that you (the `gh` user) or the flaky-test reporter opened, oldest first and one at a time, each through one Opus agent (`triage-prompt.md`) on a clean checkout of `main`. It checks the issue against main, then settles it itself: closes it as fixed or a duplicate, appends Triage / Fix / Acceptance criteria and labels it `ready-for-agent`, or splits it into `ready-for-agent` sub-issues. What needs your product call (a feature's scope, behaviour nothing settles, whether to do it at all) gets `needs-decision` and a comment with the options, its pick and the question; what needs a person's hands gets `ready-for-human`. It never changes code. An issue it can't settle goes to `needs-decision` too, so nothing is triaged twice. A triage takes a slot like a build. `TRIAGE=off` leaves `needs-triage` to you.
- **Implement:** open `ready-for-agent` issues with no open blockers and no open PR, each through
  `implement` skill → `code-review` skill + fixes → push → PR (`Closes #N`). There the issue leaves this loop.
- **Babysit:** every minute it takes each open PR that is not a draft, was opened by you (the `gh` user) from a branch of this repo (never a fork), and closes an open issue you wrote that carries `ready-for-agent`. Each goes through CI (a rebase when another merge conflicts with it, one auto-fix attempt when it fails) → squash-merge, one merge at a time. It opens a sandbox only when a PR needs an agent. PRs you open by hand qualify too.

So a crash after the PR is open never builds the issue again: the next run babysits the PR. An issue with any open PR, a draft included, is left to that PR.
At most 2 agents run at once (`MAX_PARALLEL=N` to change). A build takes a slot from claim to PR, and each rebase or CI fix takes one, ahead of new builds waiting for a slot. Waiting on CI holds none.
While a loop has an issue it carries `agent-working`; the label comes off when that loop lets go of it.
Only one factory runs per machine (`~/.cache/nama-sandcastle/factory.pid`). Factories on different machines can run at once: each claims an issue by creating `refs/claims/issue-N` on origin (a PR: `refs/claims/pr-N`, a triage: `refs/claims/triage-N`), which only one of them can do, and deletes it when done or on Ctrl-C. A run that was killed outright leaves its claims behind. The next run on the same machine takes them over; free one for another machine with `git push origin --delete refs/claims/issue-N` and remove the issue's `agent-working` label.
Anything stuck is relabelled `ready-for-human` with a comment.

It builds web tickets as well as Rust ones. The image (`Dockerfile`, on `node:26-bookworm`) holds the Rust toolchain with the `wasm32` target (linking with `mold`, at most 4 cargo jobs, line-table debug info, so two sandboxes fit in OrbStack's memory), `buf`, `gh`, Claude Code, Vite+ (`vp`) with the Node 26 and pnpm 12 that `web/` pins, and Playwright's headless Chromium. On a ticket that touches `web/`, agents run CI's web checks and screenshot the screens they changed at phone and desktop widths to check them against `DESIGN.md`. The cargo registry and the pnpm store are cached on the host in `~/.cache/nama-sandcastle/`, so each sandbox doesn't download them again.

Agents stay on their ticket's PR. What they find elsewhere they park as a `needs-triage` issue (`.sandcastle/park.md`, inlined into each prompt; never `ready-for-agent`), which the triage loop picks up. Their `GH_TOKEN` comes from `.sandcastle/.env`.

An issue with sub-issues is never built directly, and it doesn't carry `ready-for-agent` (see `docs/agents/triage-labels.md`). The factory builds its labelled sub-issues (each gets the parent as context) and closes the parent once the last one is closed.

Agents are Claude Code in Docker: triage, implement and review run `claude-opus-5-5` at effort `high`; fix (merging main, mending CI) runs `claude-sonnet-5-5` at effort `medium` and retries once on Opus if Sonnet gives up. Their skills are the repo's own: third-party ones live in `.agents/skills`, linked from `.claude/skills` and pinned in `skills-lock.json`, so each worktree brings them from `origin/main`. A skill change reaches the factory only once it's merged. The implement step also inlines `clean-code` and `ponytail`, and the review step writes the PR body with `pr`. What to write, check and review for each part of the repo lives in the repo's area skills, `.claude/skills/rust-crates` and `.claude/skills/web-app`, which every step loads for the parts a ticket touches. They pull in `rust-best-practices`, `tokio-async-code-review` and `impeccable`.

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

Unattended (one supervisor per machine, `flock`). Once the queue is drained it reruns the factory every 5 minutes (`IDLE_SECS`), so issues labelled `ready-for-agent` later get built. After an infra failure such as the Claude usage limit it waits an hour instead (`RETRY_SECS`):

    systemd-run --user --unit nama-factory --setenv=MAX_PARALLEL=3 -p WorkingDirectory=$PWD \
      -p StandardOutput=append:$PWD/.sandcastle/logs/supervisor.log -p StandardError=inherit .sandcastle/supervise.sh

`systemctl --user status nama-factory` shows it; `touch .sandcastle/STOP` ends it after the running issues finish.

Logs, all in `.sandcastle/logs/`:

- `status.md`: what is in flight in either loop (stage, PR, CI run; an issue whose PR is open shows "→ pr opened" under Done, then comes back in flight while it's babysat), what each queue (babysit, build, triage) is waiting on and why (a slot, a usage pause, a blocker, a parent, another factory's claim, a crash or STOP until the next run), and the last 20 finished issues. It's rewritten on every change, and the waiting list at least once a minute. It covers this machine's factory only.
- `events.jsonl`: one JSON event per line, the record `status.md` is rendered from (`status.ts`). It keeps the history across runs.
- `launchd.log`: one timestamped line per event, stage changes with timings, and a summary of why the run ended and what is still queued.
- `crash-issue-N.log`: the full error behind a one-line crash in `launchd.log`.
- `agent-issue-N-<step>--N.log`: what each agent did.
- `mem.log`: every 30s, any sandbox over `MEM_LOG_GIB` (default 4) GiB, with its 8 biggest processes. Read it when OrbStack runs out of memory.

`npm --prefix .sandcastle run check` checks the `status.md` renderer.

Claude usage: before picking each issue the factory reads the 5-hour and weekly usage from the rate-limit headers of a 1-token Haiku call. At 95% of the 5-hour limit (`USAGE_PAUSE`) or 92% of the weekly limit (`WEEKLY_STOP`) it starts nothing new until that window resets, while running issues carry on. Past 80% of the 5-hour limit (`SLOW_5H`) or 85% of the weekly limit (`SLOW_WEEKLY`) it starts a new issue only while fewer than `SLOW_PARALLEL` (default 1) slots are busy. `launchd.log` says when it pauses (⏸), resumes (▶), slows down (🐢) and speeds up again (🐇).

Stopping gracefully: `touch .sandcastle/STOP`. Builds already running finish with their PR, PRs already babysat finish (merged or handed back), nothing new starts, and the run exits. Later runs, such as a scheduled one, exit at once while the file exists. `rm .sandcastle/STOP` to resume.

Stopping and resuming: Ctrl-C keeps branches, commits and worktrees. The next run continues on the same `agent/issue-N` branch, or babysits its PR if it had one. Remove leftover containers with `docker ps --filter ancestor=sandcastle:nama-rust`.

Failures:

- **Agent gives up or CI stays red:** the issue is relabelled `ready-for-human`, which also takes its PR out of the babysitter's hands. The PR stays open.
- **Issue closed while it's being built:** the factory notices within a minute (and at every stage change), stops that issue's agent, closes its PR and records it as cancelled. The rest of the run carries on. Once the merge has started, closing the issue no longer stops it.
- **Babysat PR closed, or its issue closed or without `ready-for-agent`:** the babysitter lets go within a minute and leaves the PR as it is.
- **No CI run starts within 15 minutes** (GitHub delays them during incidents): an infra error, below.
- **PR merged by hand:** the factory notices within a minute, at any stage, stops that issue and records it as merged ("merged by hand"), closing finished parents as usual.
- **Infra error (Docker, auth, Claude usage limit):** nothing is relabelled. The run starts no new builds or triages but keeps babysitting PRs, each at most once more, then exits with code 1. Rerun once the problem is fixed: an issue without a PR is built again, one with a PR is only babysat.
