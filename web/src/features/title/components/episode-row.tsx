import type { Ref, RefObject } from "react";

import { RowCard } from "@/features/title/components/row-card";
import { useEpisodeVirtualizer } from "@/features/title/hooks/use-episode-virtualizer";
import { type CardArt, rowLook } from "@/features/title/lib/row-look";
import type { RowScroller } from "@/features/title/lib/season-row";
import type { RowEpisode } from "@/features/title/lib/show-row";
import { paddingFor } from "@/shared/lib/padding-for";

/**
 * Every one of a Show's `episodes` in one row, those of the page's Library, `libraryId`, and those it
 * lacks, `ref`, that scrolls sideways, snapping to each, past
 * the page's column to the screen's edges, keeping only the cards near the view in the page. It
 * takes focus, so arrow keys scroll it, opens on the card at `opening`, and `scroller` scrolls it.
 * The Show's `backdrop` stands in for a missing Still.
 */
export function EpisodeRow({
  ref,
  scroller,
  libraryId,
  episodes,
  opening,
  backdrop,
}: {
  ref: RefObject<HTMLDivElement | null>;
  scroller: Ref<RowScroller>;
  libraryId: bigint | undefined;
  episodes: RowEpisode[];
  opening: number;
  backdrop: CardArt;
}) {
  const look = rowLook(episodes, backdrop);
  const virtualizer = useEpisodeVirtualizer({
    row: ref,
    scroller,
    count: episodes.length,
    opening,
  });
  const shown = virtualizer.getVirtualItems();
  // `--tiles-before` and `--tiles-after` are declared with Home's `VirtualShelf`.
  const padding = paddingFor(shown, virtualizer.getTotalSize(), 0);
  return (
    <div
      ref={ref}
      // oxlint-disable-next-line jsx-a11y/no-noninteractive-tabindex -- a row that scrolls must take focus for keyboard users to scroll it (WCAG 2.1.1)
      tabIndex={0}
      // Overscroll stays on the row, so a swipe at its end doesn't go back a page. It snaps by
      // proximity: mandatory, a jump past the cards it shows, as to a picked Season or with End,
      // would snap back to the nearest card still in the page. Its inset is the virtualizer's
      // padding, so it holds none of its own.
      className="mx-bleed snap-x snap-proximity scroll-px-shelf-edge scrollbar-none overflow-x-auto overscroll-x-contain pt-1 pb-2 outline-none focus-visible:ring-3 focus-visible:ring-ring/50"
    >
      <ul
        aria-label="Episodes"
        className="flex w-max gap-5 ps-(--tiles-before) pe-(--tiles-after)"
        style={{ "--tiles-before": padding.above, "--tiles-after": padding.below }}
      >
        {shown.map(({ index, key }) => {
          const item = episodes[index];
          const place = { index, of: episodes.length };
          return item && <RowCard key={key} {...{ libraryId, item, place, look }} />;
        })}
      </ul>
    </div>
  );
}
