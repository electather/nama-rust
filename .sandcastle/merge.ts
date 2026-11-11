// What a PR whose CI failed while main was red needs once main is green again: main's fix, by an agent when it
// conflicts, else by GitHub. A green PR merges without this (see mergePr).
export type MergeStep = "rebase" | "update" | "merge";

// conflicts: merging main into the head conflicts. behind: main's tip isn't in the head.
export function mergeStep({ conflicts, behind }: { conflicts: boolean; behind: boolean }): MergeStep {
  if (conflicts) return "rebase"; // an agent resolves it
  if (behind) return "update"; // GitHub merges main in, no agent needed
  return "merge";
}

// What one of main's push runs says about main, from its jobs. PRs test on Linux only, so main counts as red only
// when a job a PR also runs failed: a macOS- or Windows-only failure can't be what failed a PR, and waiting on it
// held every red PR while main stayed red on those for days. "unknown": a counted job is unfinished or cancelled
// (superseded), so ask the run before it.
export type MainVerdict = "red" | "green" | "unknown";
export function mainVerdict(jobs: { name: string; conclusion: string }[]): MainVerdict {
  const counted = jobs.filter((j) => j.name !== "ci-ok" && !/macos|windows/.test(j.name));
  if (counted.some((j) => j.conclusion === "" || j.conclusion === "cancelled")) return "unknown";
  return counted.some((j) => j.conclusion === "failure" || j.conclusion === "timed_out") ? "red" : "green";
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
  console.log("merge.ts: ok");
}
