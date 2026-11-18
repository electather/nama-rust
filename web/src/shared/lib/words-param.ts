/** A search param holding words, or `undefined`. The router parses `?q=1917` as JSON, so a number is words too. */
export function wordsParam(raw: unknown) {
  return typeof raw === "string" || typeof raw === "number" ? String(raw) : undefined;
}
