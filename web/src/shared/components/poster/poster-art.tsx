import { ServerImg } from "@/shared/components/server-img";
import type { ServerImage } from "@/shared/lib/server-image";
import { posterSizes, posterTile, tileArt, tileRing } from "@/shared/lib/tile-shapes";

/**
 * A poster's art alone in its 2:3 box and hairline, at `size`: no link and no label. Without art,
 * or once it fails to load, the box is plain Muted with `title` in it, or empty at a size that
 * isn't named.
 */
export function PosterArt({
  art,
  title,
  size,
}: {
  art: ServerImage | undefined;
  title: string;
  size: keyof typeof posterSizes;
}) {
  const { box, sizes, radius, named } = posterSizes[size];
  return (
    <div
      className={`relative isolate flex items-center justify-center overflow-hidden bg-muted ${box} ${posterTile.aspect} ${radius}`}
    >
      <ServerImg
        images={[art]}
        sizes={sizes}
        className={tileArt}
        fades
        fallback={
          named ? (
            <span className="p-3 text-center text-sm font-medium text-balance text-muted-foreground">
              {title}
            </span>
          ) : null
        }
      />
      <span aria-hidden className={`${tileRing} ${radius}`} />
    </div>
  );
}
