import { useEffect, useLayoutEffect, useRef, useState } from "react";

import { scrollToSeason, seasonInView, smoothUnlessReduced } from "@/features/title/lib/season-row";

/**
 * The Season a Show's row of Episodes, `row`, is on, starting at `initial`: the Season of the card
 * at its start as it scrolls, and the one `pick` scrolls it to. While a pick's scroll passes the
 * Seasons between, the picked one stays chosen.
 */
export function useSeasonRow(initial: number) {
  const row = useRef<HTMLUListElement>(null);
  const [season, setSeason] = useState(initial);
  const picked = useRef<number>(undefined);
  // Once only: a page that suspends again, such as for a Season an Event added, reruns layout
  // effects when it shows again, which would throw the row back to where it opened.
  const opening = useRef<number | undefined>(initial);
  useLayoutEffect(() => {
    if (row.current && opening.current !== undefined) {
      scrollToSeason(row.current, opening.current, "instant");
      opening.current = undefined;
    }
  }, []);
  useEffect(() => {
    const element = row.current;
    return element ? followScroll(element, picked, setSeason) : undefined;
  }, []);
  return {
    row,
    season,
    pick: (number: number) => {
      picked.current = number;
      setSeason(number);
      if (row.current) {
        scrollToSeason(row.current, number, smoothUnlessReduced());
      }
    },
  };
}

/**
 * Sets the Season in view as `row` scrolls, once a frame, holding a `picked` one till it arrives or
 * the scroll ends. Returns what stops it.
 */
function followScroll(
  row: HTMLElement,
  picked: { current: number | undefined },
  setSeason: (season: number) => void,
) {
  let frame = 0;
  const follow = () => {
    const seen = seasonInView(row);
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
