import type { Ref } from "react";

import { EpisodeCard } from "@/features/title/components/episode-card";
import type { Episode } from "@/gen/nama/v1/browse_pb";

/**
 * Every one of a Show's `episodes` in one row that scrolls sideways, snapping to each, past the
 * page's column to the screen's edges. It takes focus, so arrow keys scroll it.
 */
export function EpisodeRow({
  ref,
  libraryId,
  episodes,
}: {
  ref: Ref<HTMLUListElement>;
  libraryId: bigint;
  episodes: Episode[];
}) {
  return (
    <ul
      ref={ref}
      // oxlint-disable-next-line jsx-a11y/no-noninteractive-tabindex -- a row that scrolls must take focus for keyboard users to scroll it (WCAG 2.1.1)
      tabIndex={0}
      aria-label="Episodes"
      // Overscroll stays on the row, so a swipe at its end doesn't go back a page.
      className="mx-bleed flex snap-x snap-mandatory scroll-px-shelf-edge scrollbar-none gap-5 overflow-x-auto overscroll-x-contain px-shelf-edge pt-1 pb-2 outline-none focus-visible:ring-3 focus-visible:ring-ring/50"
    >
      {episodes.map((episode) => (
        <EpisodeCard
          key={`${episode.seasonNumber}-${episode.episodeNumber}`}
          libraryId={libraryId}
          episode={episode}
        />
      ))}
    </ul>
  );
}
