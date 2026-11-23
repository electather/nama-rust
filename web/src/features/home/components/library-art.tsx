import type { Library } from "@/gen/nama/v1/library_pb";
import { TileArt } from "@/shared/components/tile-art";
import { backdropTileSizes } from "@/shared/lib/backdrop-tile-sizes";

/** A Library's Backdrop, 16:9, with its name over the foot of it, or a plain box without one. */
export function LibraryArt({ library }: { library: Library }) {
  return (
    // Dark whatever the page's theme, so the name keeps the scrim's contrast over any art.
    <span className="dark relative isolate flex aspect-video items-end overflow-hidden rounded-lg bg-muted">
      <TileArt
        url={library.backdropUrl}
        thumbhash={library.backdropThumbhash}
        look={{ sizes: backdropTileSizes, radius: "rounded-lg" }}
      />
      <span className="relative w-full truncate caption-scrim px-4 pt-10 pb-3 text-3xl font-medium tracking-tight text-foreground">
        {library.name}
      </span>
    </span>
  );
}
