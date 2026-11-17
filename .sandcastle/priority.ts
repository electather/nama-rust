// Builds go in order of how many open issues each one unblocks, so a merge frees the most work (oldest first on a tie).

/** Each issue → how many open issues wait on it, directly or through a chain of blockers. */
export function blockedCounts(blocks: Map<number, number[]>): Map<number, number> {
  const reach = (n: number, seen: Set<number>): Set<number> => {
    for (const m of blocks.get(n) ?? []) if (!seen.has(m)) reach(m, seen.add(m));
    return seen;
  };
  return new Map([...blocks.keys()].map((n) => {
    const seen = reach(n, new Set());
    seen.delete(n); // a cycle back to itself
    return [n, seen.size];
  }));
}

// Self-check: `npm --prefix .sandcastle run check`.
if (import.meta.main) {
  const { default: assert } = await import("node:assert/strict");
  // 1 blocks 2 and 3; 2 blocks 4 and 3 (shared, counted once); 5 blocks nothing; 6 and 7 block each other.
  const counts = blockedCounts(new Map([[1, [2, 3]], [2, [4, 3]], [5, []], [6, [7]], [7, [6]]]));
  assert.deepEqual(Object.fromEntries(counts), { 1: 3, 2: 2, 5: 0, 6: 1, 7: 1 });
  console.log("priority.ts: ok");
}
