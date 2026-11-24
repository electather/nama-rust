import { useId } from "react";

import type { RowTile } from "@/shared/components/row-poster/row-tiles";
import { TileBadge } from "@/shared/components/row-poster/tile-badge";
import { ShelfPoster } from "@/shared/components/shelf-poster";

/**
 * A row's title on its shelf: its poster alone, with its badge, opening its page. `current` marks
 * the title whose page the row is on.
 */
export function RowPoster({ tile, current = false }: { tile: RowTile; current?: boolean }) {
  const badgeId = useId();
  return (
    <ShelfPoster
      current={current}
      link={tile.link}
      title={tile.title}
      describedBy={tile.badge && badgeId}
      art={{ url: tile.posterUrl, thumbhash: tile.posterThumbhash }}
    >
      {tile.badge && <TileBadge id={badgeId} badge={tile.badge} />}
    </ShelfPoster>
  );
}
