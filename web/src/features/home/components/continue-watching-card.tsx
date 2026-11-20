import { Link } from "@tanstack/react-router";

import { CardMenu } from "@/features/home/components/card-menu";
import { ContinueWatchingCaption } from "@/features/home/components/continue-watching-caption";
import { TileArt } from "@/features/home/components/tile-art";
import type { CardActions } from "@/features/home/lib/card-actions";
import type { Card } from "@/features/home/lib/continue-watching-cards";
import { backdropTile } from "@/features/home/lib/shelf-tiles";
import { isStarted } from "@/shared/lib/watch-progress";

/** The `sizes` of a Continue Watching card's Backdrop, as `backdropTile` sizes it. */
const backdropSizes = "(max-width: 933px) 280px, (max-width: 1466px) 30vw, 440px";

/**
 * A Continue Watching card: its Backdrop, opening where it plays, with its title, how far in you
 * are and what's left over the art, and its menu to mark it watched or remove it.
 */
export function ContinueWatchingCard({ card, actions }: { card: Card; actions: CardActions }) {
  const { url, thumbhash, title, logoUrl, detail, shownDetail, watchState } = card.backdrop;
  const verb = isStarted(watchState) ? "Resume" : "Play";
  return (
    <div className={`relative isolate aspect-4/3 overflow-hidden bg-muted ${backdropTile.radius}`}>
      <TileArt
        url={url}
        thumbhash={thumbhash}
        look={{ sizes: backdropSizes, radius: backdropTile.radius }}
      />
      <div
        aria-hidden
        className="absolute inset-0 bg-linear-to-t from-scrim via-scrim/45 via-36% to-transparent to-60%"
      />
      <Link
        {...card.link}
        aria-label={[`${verb} ${title}`, detail].filter(Boolean).join(", ")}
        className={`absolute inset-0 outline-none focus-visible:outline-2 focus-visible:-outline-offset-3 focus-visible:outline-ring ${backdropTile.radius}`}
      />
      <div className="pointer-events-none absolute inset-x-1 bottom-1 flex items-end gap-2 ps-2.5">
        <ContinueWatchingCaption
          title={title}
          logoUrl={logoUrl}
          detail={shownDetail}
          state={watchState}
        />
        <CardMenu title={title} actions={actions} />
      </div>
    </div>
  );
}
