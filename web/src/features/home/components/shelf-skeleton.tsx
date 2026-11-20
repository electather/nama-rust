import { shelfBleed } from "@/features/home/lib/shelf-bleed";
import type { ShelfTile } from "@/features/home/lib/shelf-tiles";

/** Where a shelf's first tiles will be while they load: boxes of their shape, in the same row. */
export function ShelfSkeleton({
  tile,
  aspect,
}: {
  tile: ShelfTile;
  aspect: "aspect-4/3" | "aspect-2/3";
}) {
  return (
    <div aria-hidden className={`flex gap-5 overflow-hidden ${shelfBleed}`}>
      {Array.from({ length: 8 }, (_, index) => (
        <span
          key={index}
          className={`shrink-0 animate-pulse bg-muted motion-reduce:animate-none ${tile.width} ${tile.radius} ${aspect}`}
        />
      ))}
    </div>
  );
}
