import { type Virtualizer, useVirtualizer } from "@tanstack/react-virtual";
import {
  type Ref,
  type RefObject,
  useImperativeHandle,
  useLayoutEffect,
  useRef,
  useState,
} from "react";

import {
  type RowBox,
  type RowScroller,
  jumpUnsnapped,
  watchBox,
} from "@/features/title/lib/season-row";

/** The gap between a Show's Episode cards, `gap-5`. */
const GAP = 20;

/** A 4:3 card's widest, `--spacing-tile-backdrop`, and no inset, until the row shows a card. */
const GUESS: RowBox = { width: 440, inset: 0 };

/**
 * The virtualizer of a Show's row of `count` Episodes, `row`, which opens on the card at
 * `opening` and which `scroller` scrolls. Every card is as wide as the one shown, and the row's
 * inset lines the first up with the page's column, so it counts both from what the row shows,
 * rather than guessing, and a jump far along the row lands on the cards it then shows.
 */
export function useEpisodeVirtualizer({
  row,
  scroller,
  count,
  opening,
}: {
  row: RefObject<HTMLDivElement | null>;
  scroller: Ref<RowScroller>;
  count: number;
  opening: number;
}) {
  const [box, setBox] = useState<RowBox>();
  const sized = box ?? GUESS;
  // oxlint-disable-next-line react/incompatible-library -- the virtualizer re-renders the row itself whenever it scrolls
  const virtualizer = useVirtualizer({
    horizontal: true,
    count,
    getScrollElement: () => row.current,
    estimateSize: () => sized.width,
    paddingStart: sized.inset,
    paddingEnd: sized.inset,
    scrollPaddingStart: sized.inset,
    gap: GAP,
    overscan: 3,
  });
  const hasCards = virtualizer.getVirtualItems().length > 0;
  // Once it shows a card, and again as the screen's width changes them.
  useLayoutEffect(() => {
    const element = row.current;
    return element && hasCards ? watchBox(element, setBox) : undefined;
  }, [hasCards, row]);
  useLayoutEffect(() => {
    virtualizer.measure();
  }, [box, virtualizer]);
  // The virtualizer scrolls the row itself, so every scroll goes through it, or it puts back its own.
  useImperativeHandle(scroller, () => ({
    toCard: (index, behavior) => {
      const element = row.current;
      if (element && index >= 0) {
        const go = () => {
          virtualizer.scrollToIndex(index, { align: "start", behavior });
        };
        // A smooth scroll snaps where it ends, by when the row shows the cards there.
        if (behavior === "smooth") {
          go();
        } else {
          jumpUnsnapped(element, go);
        }
      }
    },
  }));
  useOpening({ row, virtualizer, box }, opening);
  return virtualizer;
}

/**
 * Scrolls the `row` to the card at `opening` once, as soon as it has a card to measure, `box`: a
 * page that suspends again reruns layout effects when it shows again, which would throw it back.
 */
function useOpening(
  {
    row,
    virtualizer,
    box,
  }: {
    row: RefObject<HTMLDivElement | null>;
    virtualizer: Virtualizer<HTMLDivElement, Element>;
    box: RowBox | undefined;
  },
  opening: number,
) {
  const opened = useRef(false);
  useLayoutEffect(() => {
    const element = row.current;
    if (box && element && !opened.current) {
      opened.current = true;
      // By the measured width: the virtualizer takes it in only on its next render.
      jumpUnsnapped(element, () => {
        virtualizer.scrollToOffset(Math.max(opening, 0) * (box.width + GAP));
      });
    }
  }, [box, opening, row, virtualizer]);
}
