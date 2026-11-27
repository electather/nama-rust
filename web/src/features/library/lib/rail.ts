/** Where a letter or year starts in a Library's listing, as its first page says. */
export interface IndexKey {
  key: string;
  position: number;
}

/** A place on the quick-scroll rail: a key to jump to, or a dot for keys with no titles. */
export type RailStop = ({ kind: "key" } & IndexKey) | { kind: "gap"; after: string };

const ALPHABET = "#ABCDEFGHIJKLMNOPQRSTUVWXYZ";

/** How many steps apart two keys are: letters on the alphabet, "#" before A, or years. */
function distance(a: string, b: string) {
  const [x, y] = [ALPHABET.indexOf(a), ALPHABET.indexOf(b)];
  return x === -1 || y === -1 ? Math.abs(Number(a) - Number(b)) : Math.abs(x - y);
}

/**
 * The rail for `index`: each of its keys in the listing's order, and one dot for each run of
 * letters or years between two of them that has no titles, so a missing key takes no row.
 */
export function railOf(index: readonly IndexKey[]): RailStop[] {
  return index.flatMap((key, at) => {
    const before = index[at - 1];
    const stop: RailStop = { kind: "key", ...key };
    if (before === undefined || !(distance(before.key, key.key) > 1)) {
      return [stop];
    }
    const gap: RailStop = { kind: "gap", after: before.key };
    return [gap, stop];
  });
}

/** What the rail's button for `key` is called: "Movies starting with A", "Movies from 2014". */
export function railKeyName(key: string, titles: string) {
  if (key === "#") {
    return `${titles} starting with a number or symbol`;
  }
  return /^\d+$/u.test(key) ? `${titles} from ${key}` : `${titles} starting with ${key}`;
}
