// A 0.2.0 Server can't trade the Hub's token for a Device, nor answer the Hub's origin (ADR 0105).
const MINIMUM_VERSION = [0, 2, 1];

/** Whether a Server reporting `version`, such as "0.3.0", is older than the page at the Hub needs. */
export function isBelowMinimumVersion(version: string) {
  const parts = version.split(/[.+-]/u).map(Number);
  for (const [at, minimum] of MINIMUM_VERSION.entries()) {
    const part = parts[at] ?? 0;
    if (part !== minimum) {
      // A part that isn't a number is never above, so a version the page can't read is too old.
      return !(part > minimum);
    }
  }
  return false;
}
