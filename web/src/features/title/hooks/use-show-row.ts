import { useMemo } from "react";

import type { SeasonAsking } from "@/features/title/hooks/season-asking";
import { useSeasonRow } from "@/features/title/hooks/use-season-row";
import { heroTarget, targetSeason } from "@/features/title/lib/hero-target";
import { openingSeason, seasonShown } from "@/features/title/lib/opening-season";
import { rowSeasons } from "@/features/title/lib/row-seasons";
import { seasonsOfRow, showRow } from "@/features/title/lib/show-row";
import type { Episode, Season } from "@/gen/nama/v1/browse_pb";
import { Permission } from "@/gen/nama/v1/sign_in_pb";
import { useMay } from "@/shared/hooks/use-may";

/** A Show to Request has no Library's Episodes, and one array keeps its row's identity. */
const NONE: Episode[] = [];

/** A Show's row of Episodes as `useShowRow` gives it, for its page's hero and `ShowEpisodes`. */
export type ShowRow = ReturnType<typeof useShowRow>;

/**
 * A Show's row: the page's Library's `episodes` and `seasons`, with the Episodes of `asking`'s
 * found Show the Library lacks, for a User who may Request Shows. It gives the row, `episodes`, its
 * `seasons`, the main button's `target`, the Episode Continue Watching has, `resumed`, else the
 * next to watch, and the `season` the row is on. It opens on `asked` if the Show has it, else the
 * target's Season, else the first, and goes back there once its Season is deleted.
 */
export function useShowRow({
  episodes = NONE,
  seasons: librarySeasons = [],
  asking,
  resumed,
  asked,
}: {
  episodes?: Episode[];
  seasons?: Season[];
  asking: SeasonAsking | undefined;
  resumed?: Episode | undefined;
  asked?: number | undefined;
}) {
  const found = asking?.show;
  const you = asking?.you;
  // One row while its Episodes don't change, so its scroll is followed once.
  const { row, tall } = useMemo(() => {
    const merged = showRow(episodes, found, you);
    return { row: merged, tall: merged.some(({ episode }) => episode.overview !== undefined) };
  }, [episodes, found, you]);
  const seasons = seasonsOfRow(row, rowSeasons(librarySeasons));
  const target = heroTarget(row, resumed, useMay(Permission.MANAGE_REQUESTS));
  const toPlay = targetSeason(target);
  const {
    season,
    row: element,
    scroller,
    opening,
    pick,
  } = useSeasonRow(openingSeason(seasons, asked, toPlay), row);
  return {
    episodes: row,
    seasons,
    target,
    season: seasonShown(seasons, season, toPlay),
    tall,
    // What `ShowEpisodes` draws and scrolls the row by.
    view: { element, scroller, opening, pick },
  };
}
