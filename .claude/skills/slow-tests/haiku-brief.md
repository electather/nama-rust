Goal: report CI job timings and the slowest macOS and Windows tests for the GitHub repo electather/nama-rust (local checkout in the working directory; use `gh`). Read-only: edit no files, push nothing, rerun nothing. Linux test durations are mined elsewhere; skip them.

Context: CI is `.github/workflows/ci.yml`. The `test` job runs cargo-nextest with profile `ci` (`.config/nextest.toml`) on Linux, macOS and Windows, each split into 2 partitions. macOS and Windows upload no reports, so read their job logs, where nextest prints `PASS [   12.345s]` and `SLOW [> 10.000s]` lines per test.

Steps:

1. `gh run list --repo electather/nama-rust --workflow CI --branch main --event push --status success --limit 10`. Take the 5 newest.
2. For each run, `gh run view <id> --json jobs` gives each job's wall time and step timings.
3. Fetch each run's macOS and Windows `test` job logs with `gh run view --job <job-id> --log` into `<scratchpad>/logs/`. On a rate limit, wait for its reset and carry on.
4. Aggregate per OS and test (binary and name): median and max duration across runs, samples seen, and any retry or flaky mark.

Report, as plain text:

- Each job's median wall time, and which job is the critical path.
- How much of each test job is build versus test execution, from step timings.
- Per OS, every test whose median is over 10 s, as a table: binary, name, median s, max s, samples, flaky.
- Every run or log you couldn't parse, and why.

Data only: no guesses about causes or fixes. Write the aggregated samples to `<scratchpad>/agg.json` and say where.
