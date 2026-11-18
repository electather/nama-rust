import type { VirtualItem } from "@tanstack/react-virtual";

/**
 * The padding that stands in for the rows above and below `rows`, so the grid keeps its height
 * and columns: rows start below `top` and the last ends at `end`, both down the page.
 */
export function paddingFor(rows: VirtualItem[], end: number, top: number) {
  const [start = top] = rows.map((row) => row.start);
  const shownEnd = rows.at(-1)?.end ?? top;
  return { above: `${start - top}px`, below: `${end - shownEnd}px` };
}
