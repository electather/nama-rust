// What a PR needs before the babysitter may merge it. A PR merges only on a green run of a head that contains
// main's tip: a run against an older main proves nothing about the new one. #546 and #555 were each green, merged
// three minutes apart without a conflict, and both added migration 0041 (#561).
export type MergeStep = "rebase" | "update" | "merge";

// conflicts: merging main into the head conflicts. behind: main's tip isn't in the head.
export function mergeStep({ conflicts, behind }: { conflicts: boolean; behind: boolean }): MergeStep {
  if (conflicts) return "rebase"; // an agent resolves it
  if (behind) return "update"; // GitHub merges main in, no agent needed
  return "merge";
}

// Self-check: `npm --prefix .sandcastle run check`.
if (import.meta.main) {
  const { default: assert } = await import("node:assert/strict");
  assert.equal(mergeStep({ conflicts: true, behind: true }), "rebase");
  assert.equal(mergeStep({ conflicts: false, behind: true }), "update", "behind but clean still runs CI again");
  assert.equal(mergeStep({ conflicts: false, behind: false }), "merge");
  console.log("merge.ts: ok");
}
