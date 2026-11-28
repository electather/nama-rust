import type { Ref, RefObject } from "react";

import { EpisodeRow } from "@/features/title/components/episode-row";
import { SeasonBar } from "@/features/title/components/season-bar";
import type { RowScroller } from "@/features/title/lib/season-row";
import type { RowEpisode } from "@/features/title/lib/show-row";
import type { ExternalId } from "@/gen/nama/v1/match_pb";
import type { ServerImage } from "@/shared/lib/server-image";

/**
 * A Show's Episodes in one row, Season after Season, Specials last, under the name of the Season
 * it's on, `current`, a menu that scrolls it to another of `seasons`. The Show's `backdrop` stands
 * in for a missing Still.
 */
export function ShowEpisodes({
  row,
  scroller,
  opening,
  show,
  seasons,
  episodes,
  current,
  onPick,
  backdrop,
}: {
  row: RefObject<HTMLDivElement | null>;
  scroller: Ref<RowScroller>;
  opening: number;
  show: { libraryId: bigint; showId: ExternalId } | undefined;
  seasons: number[];
  episodes: RowEpisode[];
  current: number;
  onPick: (season: number) => void;
  backdrop: ServerImage | undefined;
}) {
  return (
    <section aria-label="Episodes" className="flex flex-col gap-4">
      <SeasonBar {...{ seasons, current, onPick }} />
      <EpisodeRow
        ref={row}
        scroller={scroller}
        show={show}
        episodes={episodes}
        opening={opening}
        backdrop={backdrop}
      />
    </section>
  );
}
