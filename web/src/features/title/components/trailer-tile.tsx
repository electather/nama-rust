import { useState } from "react";

import { TrailerCaption } from "@/features/title/components/trailer-caption";
import { TrailerMenu } from "@/features/title/components/trailer-menu";
import { trailerHost } from "@/features/title/lib/trailer-player";
import type { Trailer } from "@/gen/nama/v1/request_pb";
import { BackdropFrame } from "@/shared/components/backdrop-card/backdrop-frame";
import { backdropLink } from "@/shared/lib/backdrop-card-classes";
import { backdropTileSizes } from "@/shared/lib/backdrop-tile-sizes";

/**
 * A Trailer's card, drawn as an Episode's: its thumbnail, or the title's `art` without one or once it
 * fails to load, under its name and host, with a "…" to Share it, linking to its host. With `onPlay`,
 * a click plays it here instead; without, it opens in a new tab.
 */
export function TrailerTile({
  trailer,
  art,
  onPlay,
}: {
  trailer: Trailer;
  art: { url: string | undefined; thumbhash: Uint8Array | undefined };
  onPlay: (() => void) | undefined;
}) {
  const { shown, onError } = useThumbnailOr(trailer, art);
  return (
    <BackdropFrame
      art={{ ...shown, sizes: backdropTileSizes, widths: "backdropWidths" }}
      caption={<TrailerCaption name={trailer.name} host={trailerHost(trailer.url)} />}
      menu={<TrailerMenu trailer={trailer} />}
      onArtError={onError}
    >
      <a
        href={trailer.url}
        target="_blank"
        rel="noopener noreferrer"
        aria-label={onPlay ? trailer.name : `${trailer.name} (opens in a new tab)`}
        aria-haspopup={onPlay ? "dialog" : undefined}
        onClick={(event) => {
          if (onPlay) {
            event.preventDefault();
            onPlay();
          }
        }}
        className={backdropLink}
      />
    </BackdropFrame>
  );
}

/** `trailer`'s thumbnail, or `art` without one or once it fails to load, as `onError` hears. */
function useThumbnailOr(
  trailer: Trailer,
  art: { url: string | undefined; thumbhash: Uint8Array | undefined },
) {
  // The Server serves only a thumbnail it stored, so showing it tells the Trailer's host nothing;
  // one not stored yet isn't found. Kept by URL, so a new thumbnail in its place gets its own try.
  const [failedUrl, setFailedUrl] = useState<string>();
  const shown =
    trailer.thumbnailUrl === undefined || trailer.thumbnailUrl === failedUrl
      ? art
      : { url: trailer.thumbnailUrl, thumbhash: trailer.thumbhash };
  return {
    shown,
    onError: () => {
      setFailedUrl(trailer.thumbnailUrl);
    },
  };
}
