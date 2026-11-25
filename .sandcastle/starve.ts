// A big PR can be rebased forever: its CI run takes 15–20 minutes, and while it runs another PR merges and
// conflicts with it, so the run is cancelled and it's rebased again. #2210's PR went stale seven times in two
// hours. Once a PR has gone stale STARVED times it's starved: while it waits on CI, other PRs' merges on this
// machine hold off (main.ts disarms their auto-merge), so it gets one clean run. The hold is bounded so a stuck
// PR can't block everyone: it ends with the starved PR's run (red, stale or merged), its factory's process, or
// HOLD_MS after its first held run, whichever comes first.
// The record is per PR on disk, shared by every run on the machine: a handover's draining run and its successor
// both merge, and a crashed run's successor keeps the count.

/** How often a PR went stale, and, while it holds others' merges, since when and for which process. */
export type StaleRecord = { stale: number; since?: number; pid?: number };

/** Times a PR goes stale before it's starved. */
export const STARVED = 3;
/** How long, in all, a starved PR may hold other merges. */
export const HOLD_MS = 30 * 60_000;

/** Whether `r`'s PR holds other PRs' merges now. */
export const holding = (r: StaleRecord, now: number, alive: (pid: number) => boolean) =>
  r.stale >= STARVED && r.pid !== undefined && alive(r.pid) && r.since !== undefined && now - r.since < HOLD_MS;

/** The starved PR `pr`'s merge waits for, if any: the one holding longest, so two starved PRs don't wait on each other. */
export function heldBy(pr: number, records: Map<number, StaleRecord>, now: number, alive: (pid: number) => boolean) {
  const first = [...records]
    .filter(([, r]) => holding(r, now, alive))
    .sort(([a, ra], [b, rb]) => ra.since! - rb.since! || a - b)[0]?.[0];
  return first === pr ? undefined : first;
}

// Self-check: `npm --prefix .sandcastle run check`.
if (import.meta.main) {
  const { default: assert } = await import("node:assert/strict");
  const live = () => true;
  const now = 100 * 60_000;
  const starved = { stale: STARVED, since: now - 60_000, pid: 1 };
  assert.equal(heldBy(1, new Map(), now, live), undefined);
  assert.equal(heldBy(1, new Map([[2210, starved]]), now, live), 2210);
  assert.equal(heldBy(2210, new Map([[2210, starved]]), now, live), undefined, "a starved PR doesn't hold itself");
  assert.equal(heldBy(1, new Map([[2210, { ...starved, stale: STARVED - 1 }]]), now, live), undefined,
    "going stale twice isn't starving");
  assert.equal(heldBy(1, new Map([[2210, { stale: 5 }]]), now, live), undefined, "between runs (rebasing, fixing red CI) it holds nothing");
  assert.equal(heldBy(1, new Map([[2210, starved]]), now, () => false), undefined, "a dead run's hold is gone");
  assert.equal(heldBy(1, new Map([[2210, { ...starved, since: now - HOLD_MS }]]), now, live), undefined, "the hold is bounded");
  const older = { ...starved, since: now - 120_000 };
  const both = new Map([[2210, starved], [2300, older]]);
  assert.equal(heldBy(2210, both, now, live), 2300, "the later starved PR waits for the earlier one");
  assert.equal(heldBy(2300, both, now, live), undefined, "and not the other way round");
  assert.equal(heldBy(1, both, now, live), 2300);
  console.log("starve.ts: ok");
}
