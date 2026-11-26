import { ShowEpisodes } from "@/features/title/components/show-episodes";
import { useSeasonRow } from "@/features/title/hooks/use-season-row";
import { openingSeason } from "@/features/title/lib/opening-season";
import { type RowEpisode, seasonsOfRow } from "@/features/title/lib/show-row";

/**
 * A found Show's row of Episodes, none in a Library this page plays from, opening on its first
 * numbered Season.
 */
export function FoundShowEpisodes({ row }: { row: RowEpisode[] }) {
  const seasons = seasonsOfRow(row);
  const {
    row: rowRef,
    scroller,
    season,
    opening,
    pick,
  } = useSeasonRow(openingSeason(seasons), row);
  return (
    <ShowEpisodes
      {...{ seasons, scroller, opening }}
      libraryId={undefined}
      row={rowRef}
      episodes={row}
      current={season}
      onPick={pick}
    />
  );
}
