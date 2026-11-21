/** `names` as a sentence lists them: "A", "A and B", "A, B and C", or past three "A, B and 2 more". */
export function andList(names: string[]): string {
  // ponytail: past three names the sentence would wrap, so the rest collapse into a count.
  const shown = names.length > 3 ? [...names.slice(0, 2), `${names.length - 2} more`] : names;
  return shown.length < 2 ? shown.join("") : `${shown.slice(0, -1).join(", ")} and ${shown.at(-1)}`;
}
