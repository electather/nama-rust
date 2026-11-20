import { useEffect, useRef } from "react";

import { SeasonChip } from "@/features/title/components/season-chip";
import { revealInRow } from "@/features/title/lib/season-row";

/**
 * A chip for each of `seasons` in one row that scrolls sideways, past the page's column to the
 * screen's edges, each scrolling the row of Episodes to that Season's first; `current` lit, and
 * scrolled into view as the Episodes move. None for a Show with one Season and no Specials.
 */
export function SeasonChips({
  seasons,
  current,
  onPick,
}: {
  seasons: number[];
  current: number;
  onPick: (season: number) => void;
}) {
  const row = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const lit = row.current?.querySelector<HTMLElement>(`button[value="${current}"]`);
    if (row.current && lit) {
      revealInRow(row.current, lit);
    }
  }, [current]);
  if (seasons.length < 2) {
    return null;
  }
  return (
    // A fieldset is as wide as its chips unless `min-w-0`, and Chrome scrolls one in an inner box
    // that ignores snapping, so a plain row inside it scrolls.
    <fieldset className="min-w-0">
      <legend className="sr-only">Seasons</legend>
      <div
        ref={row}
        // Overscroll stays on the row, so a swipe at its end doesn't go back a page.
        className="mx-bleed flex snap-x snap-mandatory scroll-px-shelf-edge scrollbar-none gap-2 overflow-x-auto overscroll-x-contain px-shelf-edge py-1"
      >
        {seasons.map((number) => (
          <SeasonChip key={number} number={number} current={number === current} onPick={onPick} />
        ))}
      </div>
    </fieldset>
  );
}
