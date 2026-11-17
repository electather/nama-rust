// Agents run in pools of slots: builds (MAX_PARALLEL), fixes (FIX_PARALLEL) and triages (TRIAGE_PARALLEL). An issue
// holds a build slot while it's built and a fix slot for each rebase or CI fix, but none while its PR waits for CI
// or its turn to merge, so a slow CI run doesn't stop the next issue from starting.
// `first` puts a waiter ahead of the others in its pool.
// `elsewhere`: slots of this pool that other runs on the machine hold. During a handover the old run drains while
// the new one starts, and the pool's size is for the machine, not per run. Their releases can't wake this pool's
// waiters, so the owner calls `wake` now and then; `changed` publishes this run's count for theirs.
export function slotPool(size: number, elsewhere = () => 0, changed = () => {}) {
  let busy = 0;
  const urgent: (() => void)[] = [];
  const normal: (() => void)[] = [];
  const free = () => size - elsewhere() - busy > 0;
  const wake = () => {
    while (free()) {
      const next = urgent.shift() ?? normal.shift();
      if (!next) return;
      busy++;
      changed();
      next();
    }
  };
  return {
    busy: () => busy,
    acquire: (first = false) => {
      if (!urgent.length && !normal.length && free()) {
        busy++;
        changed();
        return Promise.resolve();
      }
      return new Promise<void>((go) => (first ? urgent : normal).push(go));
    },
    release: () => {
      busy--;
      changed();
      wake();
    },
    wake,
  };
}

// Self-check: `npm --prefix .sandcastle run check`.
if (import.meta.main) {
  const { default: assert } = await import("node:assert/strict");
  const order: string[] = [];
  const pool = slotPool(1);
  await pool.acquire();
  void pool.acquire().then(() => order.push("new issue"));
  void pool.acquire(true).then(() => order.push("fix"));
  assert.equal(pool.busy(), 1);
  pool.release();
  await Promise.resolve();
  assert.deepEqual(order, ["fix"], "a fix goes ahead of a new issue");
  pool.release();
  await Promise.resolve();
  assert.deepEqual(order, ["fix", "new issue"]);
  pool.release();
  assert.equal(pool.busy(), 0);

  let oldRun = 2;
  const shared = slotPool(3, () => oldRun);
  await shared.acquire();
  let started = false;
  void shared.acquire().then(() => { started = true; });
  await Promise.resolve();
  assert.equal(started, false, "the draining run's two slots count against the machine's three");
  oldRun = 1;
  shared.wake();
  await Promise.resolve();
  assert.equal(started, true, "a slot the draining run freed goes to the new run");
  console.log("slots.ts: ok");
}
