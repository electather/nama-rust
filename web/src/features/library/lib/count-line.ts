/** "55 Movies", or "15 of 55 Movies" while filters keep only some: what a Library's header says. */
export function countLine(
  { shown, all }: { shown: number | undefined; all: number | undefined },
  titles: { one: string; many: string },
) {
  if (shown === undefined) {
    return "";
  }
  const name = (count: number) => (count === 1 ? titles.one : titles.many);
  if (all === undefined || all === shown) {
    return `${shown.toLocaleString()} ${name(shown)}`;
  }
  return `${shown.toLocaleString()} of ${all.toLocaleString()} ${name(all)}`;
}
