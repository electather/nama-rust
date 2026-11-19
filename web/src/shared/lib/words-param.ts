/**
 * The words in a search param, or `undefined` for none. The router parses a param as JSON, so
 * `?q=1917` arrives as a number, which is words too.
 */
export function wordsParam(raw: unknown) {
  const words = typeof raw === "string" || typeof raw === "number" ? String(raw).trim() : "";
  return words === "" ? undefined : words;
}
