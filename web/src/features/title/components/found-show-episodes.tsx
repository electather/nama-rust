import { ShowEpisodes } from "@/features/title/components/show-episodes";
import { useSeasonRow } from "@/features/title/hooks/use-season-row";
import { openingSeason } from "@/features/title/lib/opening-season";
import type { CardArt } from "@/features/title/lib/row-look";
import { type RowEpisode, seasonsOfRow } from "@/features/title/lib/show-row";

/**
 * A found Show's row of Episodes, none in a Library this page plays from, opening on its first
 * numbered Season, with the Show's `backdrop` standing in for a missing Still.
 */
export function FoundShowEpisodes({ row, backdrop }: { row: RowEpisode[]; backdrop: CardArt }) {
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
      {...{ seasons, scroller, opening, backdrop }}
      libraryId={undefined}
      row={rowRef}
      episodes={row}
      current={season}
      onPick={pick}
    />
  );
}
