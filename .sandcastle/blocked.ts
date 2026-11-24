// Why an agent's run gave up, or undefined when it didn't. A run spans up to three passes and only COMPLETE ends
// it, so a pass can say <blocked> and a later one still finish the work: #2047 was handed back with its fix
// committed. Only a <blocked> after the last COMPLETE counts.
const COMPLETE = "<promise>COMPLETE</promise>";

/** The reason from the run's last unanswered `<blocked>` marker, if any. */
export function blockedReason(stdout: string): string | undefined {
  const done = stdout.lastIndexOf(COMPLETE);
  const after = done < 0 ? stdout : stdout.slice(done + COMPLETE.length);
  return [...after.matchAll(/<blocked>(.*?)<\/blocked>/g)]
    .map((m) => m[1])
    .find((r) => r !== "one-line reason"); // ignore the prompt's own example
}

if (import.meta.main) {
  const { default: assert } = await import("node:assert/strict");
  assert.equal(blockedReason("work\n<blocked>no repro</blocked>\n"), "no repro");
  assert.equal(blockedReason(`<blocked>no repro</blocked>\nfixed it\n${COMPLETE}\n`), undefined,
    "a later pass that completes overrides an earlier blocked");
  assert.equal(blockedReason(`${COMPLETE} example\n<blocked>needs a decision</blocked>`), "needs a decision",
    "blocked after the prompt's COMPLETE example still counts");
  assert.equal(blockedReason("<blocked>one-line reason</blocked>"), undefined);
  assert.equal(blockedReason("done"), undefined);
  console.log("blocked.ts: ok");
}
