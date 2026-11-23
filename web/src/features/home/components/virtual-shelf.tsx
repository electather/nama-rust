import { useVirtualizer } from "@tanstack/react-virtual";
import { type ReactNode, useRef } from "react";

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
 * A row of `tiles` that scrolls sideways, snapping to each, past the page's column to the
 * screen's edges, keeping only the tiles near the view in the page, and loading `next` as it
 * nears its end. `setSize` is how many the whole row has, for assistive technology.
 */
export function VirtualShelf({
  tiles,
  tile,
  setSize,
  next,
}: {
  tiles: { key: string; tile: ReactNode }[];
  tile: ShelfTile;
  setSize: number | undefined;
  next?: Parameters<typeof useLoadAtEnd>[0]["next"];
}) {
  const scroller = useRef<HTMLDivElement>(null);
  // oxlint-disable-next-line react/incompatible-library -- the virtualizer re-renders this component itself whenever the shelf scrolls
  const virtualizer = useVirtualizer({
    horizontal: true,
    count: tiles.length,
    getScrollElement: () => scroller.current,
    estimateSize: () => tile.estimate,
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
