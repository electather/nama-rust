/** How many Seasons a Show has here, such as "2 Seasons"; nothing for none. */
export function seasonsOf(count: number) {
  const seasons = count === 1 ? "1 Season" : `${count} Seasons`;
  return count > 0 ? seasons : undefined;
}
