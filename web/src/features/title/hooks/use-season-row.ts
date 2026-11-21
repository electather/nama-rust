import { useEffect, useRef, useState } from "react";

import {
  type RowScroller,
  firstOfSeason,
  seasonInView,
  smoothUnlessReduced,
} from "@/features/title/lib/season-row";
import type { Episode } from "@/gen/nama/v1/browse_pb";

/**
 * The Season a Show's row of `episodes`, `row`, is on, starting at `initial`: the Season of the card
 * at its start as it scrolls, and the one `pick` scrolls it to. While a pick's scroll passes the
 * Seasons between, the picked one stays chosen. `opening` is the card the row opens on, and
 * `scroller` what the row scrolls by.
 */
export function useSeasonRow(initial: number, episodes: Episode[]) {
  const row = useRef<HTMLDivElement>(null);
  const scroller = useRef<RowScroller>(null);
  const [season, setSeason] = useState(initial);
  // The row opens on `initial`, so it holds until the row gets there.
  const picked = useRef<number | undefined>(initial);
  useEffect(() => {
    const element = row.current;
    return element ? followScroll(element, { picked, episodes }, setSeason) : undefined;
  }, [episodes]);
  return {
    row,
    scroller,
    season,
    // Only the row's first render reads it.
    opening: firstOfSeason(episodes, initial),
    pick: (number: number) => {
      picked.current = number;
      setSeason(number);
      scroller.current?.toCard(firstOfSeason(episodes, number), smoothUnlessReduced());
    },
  };
}

/**
 * Sets the Season in view as `row` of `episodes` scrolls, once a frame, holding a `picked` one till
 * it arrives or the scroll ends. Returns what stops it.
 */
function followScroll(
  row: HTMLElement,
  { picked, episodes }: { picked: { current: number | undefined }; episodes: Episode[] },
  setSeason: (season: number) => void,
) {
  let frame = 0;
  const follow = () => {
    const seen = seasonInView(row, episodes);
    // Still on its way to a pick, past the Seasons between.
    if (seen === undefined || (picked.current !== undefined && picked.current !== seen)) {
      return;
    }
    picked.current = undefined;
    setSeason(seen);
  };
  const onScroll = () => {
    cancelAnimationFrame(frame);
    frame = requestAnimationFrame(follow);
  };
  // Where it's supported, a scroll that ends short of the pick lets the row lead again.
  const onScrollEnd = () => {
    picked.current = undefined;
    follow();
  };
  row.addEventListener("scroll", onScroll, { passive: true });
  row.addEventListener("scrollend", onScrollEnd);
  return () => {
    cancelAnimationFrame(frame);
    row.removeEventListener("scroll", onScroll);
    row.removeEventListener("scrollend", onScrollEnd);
  };
}
