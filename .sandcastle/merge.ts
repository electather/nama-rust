// What a PR whose CI failed while main was red needs once main is green again: main's fix, by an agent when it
// conflicts, else by GitHub. A green PR merges without this (see mergePr).
export type MergeStep = "rebase" | "update" | "merge";

// conflicts: merging main into the head conflicts. behind: main's tip isn't in the head.
export function mergeStep({ conflicts, behind }: { conflicts: boolean; behind: boolean }): MergeStep {
  if (conflicts) return "rebase"; // an agent resolves it
  if (behind) return "update"; // GitHub merges main in, no agent needed
  return "merge";
}

// What a CI run, a PR's or main's, says from its jobs. PRs test on Linux only, so only the jobs a PR also runs
// count: a macOS- or Windows-only failure never stops a PR (on main it held every red PR for days); the main
// monitor gets it fixed instead (see mainAction). "unknown": a counted job is unfinished or cancelled (superseded).
export type Verdict = "red" | "green" | "unknown";
const failed = (j: { conclusion: string }) => j.conclusion === "failure" || j.conclusion === "timed_out";
export function runVerdict(jobs: { name: string; conclusion: string }[]): Verdict {
  const counted = jobs.filter((j) => j.name !== "ci-ok" && !/macos|windows/.test(j.name));
  if (counted.some((j) => j.conclusion === "" || j.conclusion === "cancelled")) return "unknown";
  return counted.some(failed) ? "red" : "green";
}

// The jobs that failed, on every OS. ci-ok only sums up the others.
export const failedJobs = (jobs: { name: string; conclusion: string }[]) =>
  jobs.filter((j) => j.name !== "ci-ok" && failed(j)).map((j) => j.name);

// What the main monitor does about the finished push run of main's tip. A first failure is rerun, so a flaky test
// files nothing; a second files a main-red issue, unless one is open already: its fix is on the way.
export type MainAction = "none" | "rerun" | "file";
export function mainAction({ attempt, failed }: { attempt: number; failed: string[] }, fixOpen: boolean): MainAction {
  if (!failed.length || fixOpen) return "none";
  return attempt < 2 ? "rerun" : "file";
}

// Self-check: `npm --prefix .sandcastle run check`.
if (import.meta.main) {
  const { default: assert } = await import("node:assert/strict");
  assert.equal(mergeStep({ conflicts: true, behind: true }), "rebase");
  assert.equal(mergeStep({ conflicts: false, behind: true }), "update", "behind but clean still runs CI again");
  assert.equal(mergeStep({ conflicts: false, behind: false }), "merge");
  const job = (name: string, conclusion: string) => ({ name, conclusion });
  assert.equal(runVerdict([job("test (ubuntu-latest, 1/2)", "success"), job("test (windows-latest, 1/2)", "failure"),
    job("ci-ok", "failure")]), "green", "a Windows-only failure leaves main green for PRs");
  assert.equal(runVerdict([job("test (ubuntu-latest, 1/2)", "success"), job("test (macos-latest, 2/2)", "")]),
    "green", "an unfinished macOS job doesn't hold the verdict");
  assert.equal(runVerdict([job("rust-check", "failure"), job("test (ubuntu-latest, 1/2)", "success")]), "red");
  assert.equal(runVerdict([job("test (ubuntu-latest, 2/2)", "timed_out")]), "red");
  assert.equal(runVerdict([job("rust-check", "success"), job("test (ubuntu-latest, 1/2)", "")]), "unknown");
  assert.equal(runVerdict([job("rust-check", "cancelled")]), "unknown", "a superseded run says nothing");
  assert.deepEqual(failedJobs([job("test (windows-latest, 2/2)", "failure"), job("lint", "success"), job("ci-ok", "failure")]),
    ["test (windows-latest, 2/2)"], "the monitor counts Windows and macOS");
  assert.equal(mainAction({ attempt: 1, failed: [] }, false), "none");
  assert.equal(mainAction({ attempt: 1, failed: ["lint"] }, false), "rerun", "a first failure may be flaky");
  assert.equal(mainAction({ attempt: 2, failed: ["lint"] }, false), "file");
  assert.equal(mainAction({ attempt: 2, failed: ["lint"] }, true), "none", "one main-red issue at a time");
  console.log("merge.ts: ok");
}
