import { Link } from "@tanstack/react-router";

import { TileArt } from "@/features/home/components/tile-art";
import { TileBadge } from "@/features/home/components/tile-badge";
import type { RowTile } from "@/features/home/lib/row-tiles";
import { posterTile } from "@/features/home/lib/shelf-tiles";

/** The `sizes` of a shelf's poster, as `posterTile` sizes it. */
const posterSizes = "(max-width: 1071px) 150px, (max-width: 1500px) 14vw, 210px";

/** A row's title on its shelf: its poster alone, named by its title, opening its page. */
export function RowPoster({ tile }: { tile: RowTile }) {
  return (
    <Link
      {...tile.link}
      aria-label={tile.title}
      className={`relative isolate block aspect-2/3 overflow-hidden bg-muted outline-none focus-visible:ring-3 focus-visible:ring-ring/50 ${posterTile.radius}`}
    >
      <TileArt
        url={tile.posterUrl}
        thumbhash={tile.posterThumbhash}
        look={{ sizes: posterSizes, radius: posterTile.radius }}
      />
      {tile.badge && <TileBadge badge={tile.badge} />}
    </Link>
  );
}
