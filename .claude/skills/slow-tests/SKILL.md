---
name: slow-tests
description: Mine recent CI runs for the slowest tests, then speed up, merge, move or remove every offender.
disable-model-invocation: true
---

# Slow tests

An **offender** is a test whose median CI duration is over nextest's slow threshold, `slow-timeout`'s period in `.config/nextest.toml` (10 s today). Work goes by **cause**, not by test: one cause, such as a fixture every test starts, often makes dozens of offenders, and fixing it once clears them all.

## 1. Mine

Linux test durations come from the Grafana dashboard `nama-ci-tests` through the Grafana MCP. Every CI run's nextest and vitest results land there (ADR 0073).

1. Dispatch a background agent on `model: "haiku"` with the brief in [`haiku-brief.md`](haiku-brief.md), filling in the scratchpad path. It covers what Grafana doesn't: job timings and the macOS and Windows logs.
2. Call `get_dashboard_panel_queries` with uid `nama-ci-tests` and `start: "now-7d"` for panels 1 (Main runs), 2 (Slowest tests on main) and 4 (Flaky and hung tests). Run each `processedQuery` with `query_loki_logs` on its datasource, `queryType: "instant"`. `run_panel_query` returns every step of the range instead, about ten times the output.
3. Panel 2 gives each test that ran over 10 s on main: how many runs it did (A), and the median (B) and max (C) of those runs. A test is an offender when A is over half of panel 1's run count. B leaves out the fast runs, so it can sit above the test's true median.
4. While the agent runs, read `.config/nextest.toml`, the `test` job in `.github/workflows/ci.yml` and `crates/nama-server/AGENTS.md` "Tests".

Without the Grafana MCP, or when panel 1 is empty, read the same lines from `refs/ci/test-history` (`README.md` "Flaky and slow tests") and say so. Done when the offenders are listed, the agent's table is back, and every run it couldn't parse is named.

## 2. Diagnose

For each offender, read the test, its fixture, and the code it waits on, and name its cause. Separate **contention** from **intrinsic** cost: run the test alone locally (`cargo nextest run -p <crate> --test <file> -E 'test(<name>)'`). Fast alone and slow in CI means it burns CPU while other tests run beside it. Measure that with `/usr/bin/time` over its whole test binary, as CPU-seconds per test. Prove a suspected cause with a throwaway local experiment before you fix it: revert the experiment and confirm `git status` is clean.

Causes this repo has had:

- **Cold compile.** A Server compiles every bundled Plugin at start. `TestServer` and `tests/binary.rs` share one wasmtime cache through `share_compile_cache`. A new fixture or binary test that starts a Server without it pays the compile again.
- **Real timeouts.** A test that waits out `CALL_TIMEOUT` (5 s), a Plugin's retry delay such as Sonarr's `REFRESH_RETRY`, or a `sleep` past one.
- **Heavy payloads.** Encoding or decoding multi-megapixel images or transcoding media through the whole Server.
- **Process spawns.** `tests/binary.rs` starting the real binary, often more than once a test.
- **Test Plugin builds.** `test_plugin::build` runs `cargo build` under one lock across test processes, rebuilding whenever the features change.

Done when every offender has a named cause, backed by a measurement or a line of code.

## 3. Verdict

Give each offender one verdict:

- **Fix the cause**: a shared cache, a shorter limit under the `testing` feature, a moved clock (`TestServer::advance_clock`), a smaller payload.
- **Merge**: tests that each start the same costly fixture for small checks share one start.
- **Move**: an assertion about one function becomes a unit test beside it, with no Server.
- **Trim**: drop the slow case of a table-driven test when another test checks that case.
- **Remove**: only when another test checks the same behaviour. Name that test and the assertion that covers it.
- **Keep**: it alone checks something worth its time. Say what.

Present the verdicts as a table: test, CI median, cause, verdict, and the covering test for every trim or remove. Wait for the user's go-ahead before editing.

## 4. Fix

Work in a worktree on a new branch from `origin/main` (the main checkout is shared). Fix shared causes first and measure again: an offender a shared fix brings under the threshold needs no change of its own. When a removal leaves a helper, a fake's mode or an import unused, delete it too. Follow the `rust-crates` skill for writing and checks.

Done when every changed test binary passes, the `rust-crates` checks pass, and each fix has a before-and-after measurement.

## 5. Ship

Commit and open a PR with the `pr` skill. Its Evidence is the before-and-after measurements and the CI medians from step 1. List every trimmed or removed test with the test that covers it.
