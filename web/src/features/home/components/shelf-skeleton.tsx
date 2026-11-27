import { shelfBleed } from "@/shared/lib/shelf-bleed";
import { type TileShape, tileGap } from "@/shared/lib/tile-shapes";

/** Where a shelf's first tiles will be while they load: boxes of their shape, in the same row. */
export function ShelfSkeleton({ tile }: { tile: TileShape }) {
  return (
    <div aria-hidden className={`flex overflow-hidden ${tileGap.className} ${shelfBleed}`}>
      {Array.from({ length: 8 }, (_, index) => (
        <span
          key={index}
          className={`shrink-0 animate-pulse bg-muted motion-reduce:animate-none ${tile.widthClass} ${tile.radius} ${tile.aspect}`}
          style={{ "--tile-width": tile.width }}
        />
      ))}
    </div>
  );
}
