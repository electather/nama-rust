# Test history keeps 60 days in one commit, and Grafana Cloud gets a copy

ADR 0022 kept every CI run's test results forever, one file per month on the orphan branch `test-history`. CI now writes about 25k lines a day, so `2026-11.jsonl` passed GitHub's 100 MB file limit on 2026-11-12, and every push after it was refused. This amends ADR 0022: the history is one file per day, `YYYY-MM-DD.jsonl`, and holds only the last 60 days. Each append builds a commit with no parent from those files and pushes it with `--force-with-lease`, so the history is always one commit and old days leave the repository rather than only its tree. It lives on the ref `refs/ci/test-history`, not a branch: a clone fetches only `refs/heads/*` and tags, so it no longer downloads about 80 MB of history it never reads. The flaky check still reads the last 30 days from it, and the definition of flaky is unchanged.

`test-report` also sends each run's lines to Grafana Cloud as OTLP logs, after it has appended to the branch and updated the flaky-test issues, so slow and flaky tests can be explored in a dashboard. The branch stays the record; Grafana is a copy that may lose a run without harm. Each log's body is the history line as written, under `service.name=nama-ci-tests`; `outcome` and `branch` go in as structured metadata, never as index labels, since every PR branch would be a new stream. The workflow holds a token that can only write logs, and skips the send when the secret is missing.

## Considered Options

- **Grafana Cloud as the only store**: rejected. Its free tier keeps logs about 14 days, shorter than the flaky check's 30, and reading them back needs a second token and a LogQL query in the workflow.
- **Monthly files, gzipped**: rejected. Each append would rewrite a compressed file, and the branch would still grow forever.
- **Delete files past 60 days in ordinary commits**: rejected. Git keeps every deleted blob, so the branch, and every clone that fetches it, would still grow forever.
- **Record only failures, flakes and hangs**: rejected. A flake is a pass and a failure on one tree, so the check needs the passes.
- **A scheduled job that squashes the branch**: rejected. It would be a second writer racing the appends, for what each append can do itself.

## Consequences

- A run older than 60 days is gone from the history; a question about older history has no answer.
- GitHub's web UI shows only branches and tags, so reading the history takes `git fetch origin refs/ci/test-history:refs/ci/test-history`.
- Two appends can race. The loser's lease fails, so it fetches the winner's commit, adds its lines and pushes again, as before.
- A failed Grafana send leaves a warning, not a failed run, and a re-run of an already recorded run sends nothing, so that run is missing from Grafana for good.
