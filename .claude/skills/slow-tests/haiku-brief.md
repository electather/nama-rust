Goal: list the slowest tests in CI for the GitHub repo electather/nama-rust (local checkout in the working directory; use `gh`). Read-only: edit no files, push nothing, rerun nothing.

Context: CI is `.github/workflows/ci.yml`. The `test` job runs cargo-nextest with profile `ci` (`.config/nextest.toml`) on Linux, macOS and Windows, each split into 2 partitions. Linux uploads JUnit reports as artifacts named `test-reports-<attempt>-nextest-<partition>` (`target/nextest/*/junit.xml`); macOS and Windows upload none, so read their job logs, where nextest prints `PASS [   12.345s]` and `SLOW [> 10.000s]` lines per test. The `web-test` job runs nextest profile `ci-web`; the `web` job runs vitest.

Steps:

1. `gh run list --repo electather/nama-rust --workflow CI --branch main --event push --status success --limit 10`. Take the 5 newest.
2. For each run, `gh run view <id> --json jobs` gives each job's wall time and step timings.
3. Download each run's JUnit artifacts with `gh run download <id> --pattern 'test-reports-*' --dir <scratchpad>/ci-junit/<id>/`. Fetch macOS and Windows logs with `gh run view --job <job-id> --log` into `<scratchpad>/logs/`. On a rate limit, wait for its reset and carry on.
4. Aggregate per test (binary and name): median and max duration across every OS and run, samples seen, and any retry or flaky mark.

Report, as plain text:

- Each job's median wall time, and which job is the critical path.
- How much of each test job is build versus test execution, from step timings.
- Every test whose median is over 10 s, plus the next 10 below it, as a table: binary, name, median s, max s, samples, flaky.
- Every test nextest marked flaky, with how many runs.
- Every run or log you couldn't parse, and why.

Data only: no guesses about causes or fixes. Write the aggregated samples to `<scratchpad>/agg.json` and say where.
