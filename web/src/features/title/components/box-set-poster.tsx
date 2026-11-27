import type { BoxSetTile } from "@/features/title/lib/box-set-tiles";
import { RowPoster } from "@/shared/components/row-poster/row-poster";
import { posterTile } from "@/shared/lib/tile-shapes";

/** One Movie of a Box Set: its poster, as Home's rows draw it, opening its page or Request page. */
export function BoxSetPoster({ tile }: { tile: BoxSetTile }) {
  return (
    <li
      className={`shrink-0 snap-start ${posterTile.widthClass}`}
      style={{ "--tile-width": posterTile.width }}
    >
      <RowPoster tile={tile.tile} current={tile.current} />
    </li>
  );
}
