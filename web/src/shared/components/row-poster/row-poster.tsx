import { useId } from "react";

import { Poster } from "@/shared/components/poster/poster";
import type { RowTile } from "@/shared/components/row-poster/row-tiles";
import { TileBadge } from "@/shared/components/row-poster/tile-badge";

/**
 * A row's title on its shelf: its poster alone, with its badge, opening its page. `current` marks
 * the title whose page the row is on.
 */
export function RowPoster({ tile, current = false }: { tile: RowTile; current?: boolean }) {
  const badgeId = useId();
  return (
    <Poster
      current={current}
      link={tile.link}
      title={tile.title}
      detail={tile.year}
      describedBy={tile.badge && badgeId}
      art={tile.poster}
    >
      {tile.badge && <TileBadge id={badgeId} badge={tile.badge} />}
    </Poster>
  );
}
