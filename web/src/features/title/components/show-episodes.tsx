import type { Ref } from "react";

import { EpisodeRow } from "@/features/title/components/episode-row";
import { SeasonChips } from "@/features/title/components/season-chips";
import type { Episode } from "@/gen/nama/v1/browse_pb";

/**
 * A Show's Episodes in one row, Season after Season, Specials last, under a chip for each Season
 * that scrolls the row to it, `current` lit.
 */
export function ShowEpisodes({
  row,
  libraryId,
  seasons,
  episodes,
  current,
  onPick,
}: {
  row: Ref<HTMLUListElement>;
  libraryId: bigint;
  seasons: number[];
  episodes: Episode[];
  current: number;
  onPick: (season: number) => void;
}) {
  return (
    <section aria-label="Episodes and Seasons" className="flex flex-col gap-5">
      <SeasonChips seasons={seasons} current={current} onPick={onPick} />
      <EpisodeRow ref={row} libraryId={libraryId} episodes={episodes} />
    </section>
  );
}
