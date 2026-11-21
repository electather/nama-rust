import type { Ref, RefObject } from "react";

import { EpisodeRow } from "@/features/title/components/episode-row";
import { SeasonBar } from "@/features/title/components/season-bar";
import type { RowScroller } from "@/features/title/lib/season-row";
import type { Episode } from "@/gen/nama/v1/browse_pb";

/**
 * A Show's Episodes in one row, Season after Season, Specials last, under the name of the Season
 * it's on, `current`, a menu that scrolls it to another of `seasons`.
 */
export function ShowEpisodes({
  row,
  scroller,
  opening,
  libraryId,
  seasons,
  episodes,
  current,
  onPick,
}: {
  row: RefObject<HTMLDivElement | null>;
  scroller: Ref<RowScroller>;
  opening: number;
  libraryId: bigint;
  seasons: number[];
  episodes: Episode[];
  current: number;
  onPick: (season: number) => void;
}) {
  return (
    <section aria-label="Episodes" className="flex flex-col gap-4">
      <SeasonBar {...{ seasons, current, onPick }} />
      <EpisodeRow
        ref={row}
        scroller={scroller}
        libraryId={libraryId}
        episodes={episodes}
        opening={opening}
      />
    </section>
  );
}
