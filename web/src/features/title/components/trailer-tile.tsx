import { TrailerCaption } from "@/features/title/components/trailer-caption";
import { TrailerMenu } from "@/features/title/components/trailer-menu";
import { trailerHost } from "@/features/title/lib/trailer-player";
import type { Trailer } from "@/gen/nama/v1/request_pb";
import { BackdropFrame } from "@/shared/components/backdrop-card/backdrop-frame";
import { backdropLink } from "@/shared/lib/backdrop-card-classes";
import { type ServerImage, thumbnailOf } from "@/shared/lib/server-image";
import { backdropTile } from "@/shared/lib/tile-shapes";

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
  art: ServerImage | undefined;
  onPlay: (() => void) | undefined;
}) {
  return (
    <BackdropFrame
      // The Server serves only a thumbnail it stored, so showing it tells the Trailer's host
      // nothing; one not stored yet isn't found.
      art={{ images: [thumbnailOf(trailer), art], sizes: backdropTile.sizes }}
      caption={<TrailerCaption name={trailer.name} host={trailerHost(trailer.url)} />}
      menu={<TrailerMenu trailer={trailer} />}
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
