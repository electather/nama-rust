import { Film } from "lucide-react";

import type { BoxSet } from "@/gen/nama/v1/browse_pb";
import type { ExternalId } from "@/gen/nama/v1/match_pb";
import { type RowTile, rowTile } from "@/shared/components/row-poster/row-tiles";
import type { Shown } from "@/shared/components/state-label";
import { rowTitleKey, rowTitles } from "@/shared/lib/row-titles";
import { titleParam } from "@/shared/lib/title-param";

/** A Box Set's Movie as its row or page shows it, `current` when it's the Movie whose page it's on. */
export interface BoxSetTile {
  key: string;
  tile: RowTile;
  current: boolean;
}

/** What marks the Movie whose page the row is on, as a missing Movie's state marks it. */
const thisMovie: Shown = { label: "This Movie", Icon: Film };

/** The tiles of `boxSet`'s Movies, in its order, marking the Movie `current`, if any. */
export function boxSetTiles(boxSet: BoxSet, current?: ExternalId): BoxSetTile[] {
  const here = current && titleParam(current);
  return rowTitles(boxSet.movies).map((title) => {
    const tile = rowTile(title);
    const isCurrent = title.case === "movie" && titleParam(title.value.id) === here;
    return {
      key: rowTitleKey(title),
      tile: isCurrent ? { ...tile, badge: thisMovie } : tile,
      current: isCurrent,
    };
  });
}
