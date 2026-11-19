import type { Virtualizer } from "@tanstack/react-virtual";
import type { RefObject } from "react";

import { EpisodeListItem } from "@/features/player/components/episode-list-item";
import type { EpisodeItem } from "@/features/player/lib/episode-items";
import type { InShow } from "@/features/player/lib/in-show";
import { paddingFor } from "@/shared/lib/padding-for";

/**
 * Every Episode of the Show in one scrolling list, of which only the rows near the view are in
 * the page, however long the Show runs.
 */
export function VirtualEpisodes({
  items,
  scroll: { scroller, virtualizer, release },
  ...row
}: {
  items: EpisodeItem[];
  scroll: {
    scroller: RefObject<HTMLDivElement | null>;
    virtualizer: Virtualizer<HTMLDivElement, Element>;
    release: () => void;
  };
  inShow: InShow;
  played: number;
  onPick: () => void;
}) {
  const rows = virtualizer.getVirtualItems();
  const padding = paddingFor(rows, virtualizer.getTotalSize(), 0);
  return (
    <div
      ref={scroller}
      onWheel={release}
      onTouchMove={release}
      onPointerDown={release}
      // Proximity, not mandatory: a row the list is scrolled partway past stays reachable.
      className="max-h-120 min-h-0 snap-y snap-proximity scroll-pt-1 scrollbar-quiet overflow-y-auto px-2"
    >
      <ul
        className="flex flex-col gap-1.5 pt-(--rows-above) pb-(--rows-below)"
        style={{ "--rows-above": padding.above, "--rows-below": padding.below }}
      >
        {rows.map(({ index, key }) => {
          const item = items[index];
          return (
            item !== undefined && (
              <EpisodeListItem
                key={key}
                item={item}
                index={index}
                ref={virtualizer.measureElement}
                {...row}
              />
            )
          );
        })}
      </ul>
    </div>
  );
}
