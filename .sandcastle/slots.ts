// Agents run in pools of slots: builds (MAX_PARALLEL), fixes (FIX_PARALLEL) and triages (TRIAGE_PARALLEL). An issue
// holds a build slot while it's built and a fix slot for each rebase or CI fix, but none while its PR waits for CI
// or its turn to merge, so a slow CI run doesn't stop the next issue from starting.
// `first` puts a waiter ahead of the others in its pool.
export function slotPool(size: number) {
  let free = size;
  const urgent: (() => void)[] = [];
  const normal: (() => void)[] = [];
  return {
    busy: () => size - free,
    acquire: (first = false) =>
      free > 0 ? (free--, Promise.resolve()) : new Promise<void>((go) => (first ? urgent : normal).push(go)),
    // A waiter takes over the slot directly, so `free` stays the same.
    release: () => {
      const next = urgent.shift() ?? normal.shift();
      if (next) next();
      else free++;
    },
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
  console.log("slots.ts: ok");
}
