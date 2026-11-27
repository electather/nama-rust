import { useElementScrollRestoration } from "@tanstack/react-router";
import { useVirtualizer } from "@tanstack/react-virtual";
import { type ReactNode, useMemo, useRef } from "react";

import { useLoadAtEnd } from "@/features/home/hooks/use-load-at-end";
import type { ShelfTile } from "@/features/home/lib/shelf-tiles";
import { paddingFor } from "@/shared/lib/padding-for";
import { shelfBleed } from "@/shared/lib/shelf-bleed";

declare module "react" {
  interface CSSProperties {
    /** `VirtualShelf`'s padding standing in for the tiles before those it shows. */
    "--tiles-before"?: string;
    /** As `--tiles-before`, for the tiles after. */
    "--tiles-after"?: string;
  }
}

/** The gap between tiles, `gap-5`. */
const GAP = 20;

/**
 * How wide `tile` is on this screen, its CSS width measured in a probe, or its guess where nothing
 * is laid out, as in tests.
 */
function tileWidth(tile: ShelfTile) {
  const probe = document.createElement("div");
  probe.className = `invisible absolute ${tile.width}`;
  document.body.append(probe);
  const { width } = probe.getBoundingClientRect();
  probe.remove();
  return width > 0 ? width : tile.estimate;
}

/** Where the shelf `restoreId` names was scrolled when its page was last left, or its start. */
function useRestoredOffset(restoreId: string) {
  return useElementScrollRestoration({ id: restoreId })?.scrollX ?? 0;
}

/**
 * A row of `tiles` that scrolls sideways, snapping to each, past the page's column to the
 * screen's edges, keeping only the tiles near the view in the page, and loading `next` as it
 * nears its end. `setSize` is how many the whole row has, for assistive technology. `restoreId`
 * names it in its page, to come back to where it was scrolled: unique there, and the same each visit.
 */
export function VirtualShelf({
  tiles,
  tile,
  setSize,
  restoreId,
  next,
}: {
  tiles: { key: string; tile: ReactNode }[];
  tile: ShelfTile;
  setSize: number | undefined;
  restoreId: string;
  next?: Parameters<typeof useLoadAtEnd>[0]["next"];
}) {
  const scroller = useRef<HTMLDivElement>(null);
  // The router saves where each shelf was scrolled, but restores it only to shelves already in the
  // page as it renders, and on Back these mount later, as their rows load or come near the view.
  // So a shelf starts where it was itself. Its tiles' width must be exact for that: the virtualizer
  // never measures those before the view, and correcting a guess for them pulls it a tile back.
  const restored = useRestoredOffset(restoreId);
  const estimate = useMemo(() => tileWidth(tile), [tile]);
  // oxlint-disable-next-line react/incompatible-library -- the virtualizer re-renders this component itself whenever the shelf scrolls
  const virtualizer = useVirtualizer({
    horizontal: true,
    count: tiles.length,
    getScrollElement: () => scroller.current,
    initialOffset: restored,
    // Until it measures the scroller, the virtualizer shows no tiles, and a shelf restored past its
    // start renders a frame without them. The page is a shelf shorter then, so the page's own
    // restored scroll gets clamped. The shelf bleeds to the screen's edges, so it's about as wide.
    initialRect: { width: globalThis.innerWidth, height: 0 },
    estimateSize: () => estimate,
    getItemKey: (index) => tiles[index]?.key ?? index,
    gap: GAP,
    // ponytail: the virtualizer counts from the scroller's padding edge, not its `shelf-edge` inset,
    // so its window sits up to a tile off; overscan covers it. Pass `paddingStart` if it shows.
    overscan: 3,
  });
  const shown = virtualizer.getVirtualItems();
  const padding = paddingFor(shown, virtualizer.getTotalSize(), 0);
  useLoadAtEnd({ shown: shown.at(-1)?.index ?? -1, count: tiles.length, next });
  return (
    <div
      ref={scroller}
      data-scroll-restoration-id={restoreId}
      // Overscroll stays on the shelf, so a swipe at its end doesn't go back a page.
      className={`snap-x snap-mandatory scroll-px-shelf-edge scrollbar-none overflow-x-auto overscroll-x-contain ${shelfBleed}`}
    >
      <ul
        className="flex w-max gap-5 ps-(--tiles-before) pe-(--tiles-after)"
        style={{ "--tiles-before": padding.above, "--tiles-after": padding.below }}
      >
        {shown.map(({ index, key }) => (
          <li
            key={key}
            ref={virtualizer.measureElement}
            data-index={index}
            aria-setsize={setSize}
            aria-posinset={index + 1}
            className={`relative shrink-0 snap-start ${tile.width}`}
          >
            {tiles[index]?.tile}
          </li>
        ))}
      </ul>
    </div>
  );
}
