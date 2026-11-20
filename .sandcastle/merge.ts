// What a PR whose CI failed while main was red needs once main is green again: main's fix, by an agent when it
// conflicts, else by GitHub. A green PR merges without this (see mergePr).
export type MergeStep = "rebase" | "update" | "merge";

// conflicts: merging main into the head conflicts. behind: main's tip isn't in the head.
export function mergeStep({ conflicts, behind }: { conflicts: boolean; behind: boolean }): MergeStep {
  if (conflicts) return "rebase"; // an agent resolves it
  if (behind) return "update"; // GitHub merges main in, no agent needed
  return "merge";
}

// What one of main's push runs says about main. PRs test on Linux only, so main counts as red only when a job a PR
// also runs failed: a macOS- or Windows-only failure can't be what failed a PR, and waiting on it held every red PR
// while main stayed red on those for days. The main monitor gets those fixed instead (see mainAction).
// "unknown": a counted job is unfinished or cancelled (superseded), so ask the run before it.
export type Verdict = "red" | "green" | "unknown";
type Job = { name: string; conclusion: string };
const failed = (j: Job) => j.conclusion === "failure" || j.conclusion === "timed_out";
const verdict = (counted: Job[]): Verdict =>
  counted.some((j) => j.conclusion === "" || j.conclusion === "cancelled") ? "unknown"
  : counted.some(failed) ? "red" : "green";
export const mainVerdict = (jobs: Job[]) => verdict(jobs.filter((j) => j.name !== "ci-ok" && !/macos|windows/.test(j.name)));
// What a PR's run says: every job counts, ci-ok included, which main's ruleset requires to merge.
export const prVerdict = (jobs: Job[]) => verdict(jobs);

// The jobs that failed, on every OS. ci-ok only sums up the others.
export const failedJobs = (jobs: Job[]) =>
  jobs.filter((j) => j.name !== "ci-ok" && failed(j)).map((j) => j.name);

// What the main monitor does about main's newest finished push run. A first failure is rerun, so a flaky test
// files nothing; a second files a main-red issue, unless one is open already: its fix is on the way. So does a job
// that also failed in the finished run before it: two commits failing alike isn't a flake, and while merges land
// faster than CI a rerun's result comes after a newer run's, so waiting on it never files (#1274).
export type MainAction = "none" | "rerun" | "file";
export function mainAction(
  { attempt, failed, failedBefore }: { attempt: number; failed: string[]; failedBefore: string[] },
  fixOpen: boolean,
): MainAction {
  if (!failed.length || fixOpen) return "none";
  return attempt >= 2 || failed.some((job) => failedBefore.includes(job)) ? "file" : "rerun";
}

// Self-check: `npm --prefix .sandcastle run check`.
if (import.meta.main) {
  const { default: assert } = await import("node:assert/strict");
  assert.equal(mergeStep({ conflicts: true, behind: true }), "rebase");
  assert.equal(mergeStep({ conflicts: false, behind: true }), "update", "behind but clean still runs CI again");
  assert.equal(mergeStep({ conflicts: false, behind: false }), "merge");
  const job = (name: string, conclusion: string) => ({ name, conclusion });
  assert.equal(mainVerdict([job("test (ubuntu-latest, 1/2)", "success"), job("test (windows-latest, 1/2)", "failure"),
    job("ci-ok", "failure")]), "green", "a Windows-only failure leaves main green for PRs");
  assert.equal(mainVerdict([job("test (ubuntu-latest, 1/2)", "success"), job("test (macos-latest, 2/2)", "")]),
    "green", "an unfinished macOS job doesn't hold the verdict");
  assert.equal(mainVerdict([job("rust-check", "failure"), job("test (ubuntu-latest, 1/2)", "success")]), "red");
  assert.equal(mainVerdict([job("test (ubuntu-latest, 2/2)", "timed_out")]), "red");
  assert.equal(mainVerdict([job("rust-check", "success"), job("test (ubuntu-latest, 1/2)", "")]), "unknown");
  assert.equal(mainVerdict([job("rust-check", "cancelled")]), "unknown", "a superseded run says nothing");
  assert.equal(prVerdict([job("test (ubuntu-latest, 1/2)", "success"), job("ci-ok", "")]), "unknown", "a PR waits for ci-ok");
  assert.equal(prVerdict([job("test (windows-latest, 1/2)", "failure"), job("ci-ok", "failure")]), "red");
  assert.deepEqual(failedJobs([job("test (windows-latest, 2/2)", "failure"), job("lint", "success"), job("ci-ok", "failure")]),
    ["test (windows-latest, 2/2)"], "the monitor counts Windows and macOS");
  assert.equal(mainAction({ attempt: 1, failed: [], failedBefore: ["lint"] }, false), "none");
  assert.equal(mainAction({ attempt: 1, failed: ["lint"], failedBefore: [] }, false), "rerun", "a first failure may be flaky");
  assert.equal(mainAction({ attempt: 2, failed: ["lint"], failedBefore: [] }, false), "file");
  assert.equal(mainAction({ attempt: 1, failed: ["lint"], failedBefore: ["lint"] }, false), "file",
    "the same job failing on two commits in a row isn't a flake");
  assert.equal(mainAction({ attempt: 1, failed: ["lint"], failedBefore: ["rust-check"] }, false), "rerun");
  assert.equal(mainAction({ attempt: 2, failed: ["lint"], failedBefore: ["lint"] }, true), "none", "one main-red issue at a time");
  console.log("merge.ts: ok");
}
