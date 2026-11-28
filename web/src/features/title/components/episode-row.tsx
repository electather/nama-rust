import { EpisodeCard } from "@/features/title/components/episode-card";
import { useEpisodeVirtualizer } from "@/features/title/hooks/use-episode-virtualizer";
import type { ShowRow } from "@/features/title/hooks/use-show-row";
import type { RowLook } from "@/features/title/lib/row-look";
import type { ShowInLibrary } from "@/features/title/lib/show-params";
import { paddingFor } from "@/shared/lib/padding-for";
import { tileGap } from "@/shared/lib/tile-shapes";

/**
 * Every one of a Show's Episodes in one row, those of the page's Show, `show`, and those it
 * lacks, that scrolls sideways, snapping to each, past the page's column to the screen's edges,
 * keeping only the cards near the view in the page. It takes focus, so arrow keys scroll it, and
 * opens on the card `useShowRow` picked. Every card is drawn as its `look` says.
 */
export function EpisodeRow({
  row: {
    episodes,
    view: { element: ref, scroller, opening },
  },
  show,
  look,
}: {
  row: ShowRow;
  show: ShowInLibrary | undefined;
  look: RowLook;
}) {
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
        className={`flex w-max ps-(--tiles-before) pe-(--tiles-after) ${tileGap.className}`}
        style={{ "--tiles-before": padding.above, "--tiles-after": padding.below }}
      >
        {shown.map(({ index, key }) => {
          const item = episodes[index];
          const place = { index, of: episodes.length };
          return item && <EpisodeCard key={key} {...{ show, item, place, look }} />;
        })}
      </ul>
    </div>
  );
}
