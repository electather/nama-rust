// Agents run in MAX_PARALLEL slots. An issue holds one while it's built and again for each fix, but not while its
// PR waits for CI or its turn to merge, so a slow CI run doesn't stop the next issue from starting.
// A fix goes ahead of a new build in the queue, because finishing a PR matters more than starting another issue.
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
