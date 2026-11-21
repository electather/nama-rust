/** `entries`' distinct ones, in order, joined with commas, such as "English, French"; or nothing. */
export function listed(entries: (string | undefined)[]) {
  const distinct = [...new Set(entries.filter((entry) => entry !== undefined))];
  return distinct.length > 0 ? distinct.join(", ") : undefined;
}
