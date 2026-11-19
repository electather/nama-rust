import { useVirtualizer } from "@tanstack/react-virtual";
import { useLayoutEffect, useRef, useState } from "react";

import { type EpisodeItem, seasonShown } from "@/features/player/lib/episode-items";

/** An Episode row's height: its 112px-wide Still, 63px tall, and the row's 8px padding. */
const ROW = 79;
/** A Season heading's height. */
const HEADING = 36;
/** The list's `gap-1.5`, between rows. */
const GAP = 6;

/** Glides to a row, unless the system asks for less motion. */
function glide() {
  return globalThis.matchMedia("(prefers-reduced-motion: reduce)").matches ? "auto" : "smooth";
}

/**
 * Keeps only the rows of `items` near the scroller's view in the page, starting on the one at
 * `playing`. `show` scrolls to a Season's start, and `shown` is the Season the tabs mark: the one
 * picked, until the list is scrolled by hand, then the one at the top.
 */
export function useEpisodeScroll(items: EpisodeItem[], playing: number) {
  const scroller = useRef<HTMLDivElement>(null);
  const [picked, setPicked] = useState(items[playing]?.season);
  // oxlint-disable-next-line react/incompatible-library -- the virtualizer re-renders this component itself whenever the list scrolls
  const virtualizer = useVirtualizer({
    count: items.length,
    getScrollElement: () => scroller.current,
    estimateSize: (index) => (items[index]?.episode === undefined ? HEADING : ROW),
    gap: GAP,
    paddingStart: 4,
    // The scroller's `scroll-pt-1`, so a row it scrolls to rests where the list snaps it.
    scrollPaddingStart: 4,
    paddingEnd: 8,
    overscan: 4,
  });
  // Before the first paint and at once, so the panel opens already on the one playing.
  useLayoutEffect(() => {
    virtualizer.scrollToIndex(playing, { align: "start" });
  }, [virtualizer, playing]);
  return {
    scroller,
    virtualizer,
    shown: seasonShown(items, virtualizer.range?.startIndex, picked),
    show: (season: number) => {
      setPicked(season);
      virtualizer.scrollToIndex(
        items.findIndex((item) => item.season === season),
        { align: "start", behavior: glide() },
      );
    },
    // A wheel, touch or drag scrolls by hand: our own scrolling fires none of them.
    release: () => {
      setPicked(undefined);
    },
  };
}
