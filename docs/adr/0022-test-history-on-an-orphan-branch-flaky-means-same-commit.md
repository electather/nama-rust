# Test history lives on an orphan branch, and flaky means passed and failed on one commit

Every CI run's test results go into a history that's kept forever, so flaky and slow tests show up across runs. A `workflow_run` workflow, `test-report`, reads the run's test reports as data and appends one JSON line per test to a month's file on the orphan branch `test-history`. Git and `jq` are all it takes to read it. The workflow alone holds the write token: CI stays read-only, as it must for Dependabot PRs, and PR code never runs next to write access.

A test is flaky when its results on one commit hold both a pass and a failure: through a retry within a run, a rerun of the job, or the PR run and the `main` run of the same commit.

"One commit" means one tree: the code CI tested. A PR's run tests the PR merged into `main`, not its head, and a squash or rebase merge gives `main` a new commit, so the PR run and the `main` run share a tree but never a commit. A PR run started after `main` moved tests other code on the same head, so keying on the head would count a fix on `main` as a flake. CI writes the tree next to its test reports. A PR controls that file, but it controls its reports too, so it can't fake more than it already could.

## Considered Options

- **Actions artifacts as the history**: rejected. They expire, and collecting months of them means one download per run.
- **A SQLite file or a service in the home lab**: rejected. CI's runners are isolated from the LAN, and a service is one more thing to run and back up.
- **A paid test-analytics service**: rejected. It costs money for what a branch and a small tool do.
- **Flaky as a flip between consecutive `main` commits**: rejected. A real break fixed by the next commit would count as a flake.

## Consequences

- The history grows by one line per test per run and is never pruned.
- Two `test-report` runs can push at once, so the loser fetches and appends again; a concurrency group would cancel queued runs and lose their results.
- Cancelled CI runs and runs whose tests never built are left out, so a partial run never counts as a pass or a fail.
