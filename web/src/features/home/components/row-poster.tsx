import { TileBadge } from "@/features/home/components/tile-badge";
import type { RowTile } from "@/features/home/lib/row-tiles";
import { ShelfPoster } from "@/shared/components/shelf-poster";

/** A row's title on its shelf: its poster alone, with its badge, opening its page. */
export function RowPoster({ tile }: { tile: RowTile }) {
  return (
    <ShelfPoster
      link={tile.link}
      title={tile.title}
      art={{ url: tile.posterUrl, thumbhash: tile.posterThumbhash }}
    >
      {tile.badge && <TileBadge badge={tile.badge} />}
    </ShelfPoster>
  );
}
