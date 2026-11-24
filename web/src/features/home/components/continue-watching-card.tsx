import { CardMenu } from "@/features/home/components/card-menu";
import { ContinueWatchingCaption } from "@/features/home/components/continue-watching-caption";
import type { CardActions } from "@/features/home/lib/card-actions";
import type { Card } from "@/features/home/lib/continue-watching-cards";
import { BackdropCard } from "@/shared/components/backdrop-card/backdrop-card";
import { backdropTileSizes } from "@/shared/lib/backdrop-tile-sizes";
import { isStarted } from "@/shared/lib/watch-progress";

/**
 * A Continue Watching card: its art, opening where it plays, with its title (unless the art carries
 * it), how far in you are and what's left over the art, and its menu to mark it watched or remove it.
 */
export function ContinueWatchingCard({ card, actions }: { card: Card; actions: CardActions }) {
  const { url, thumbhash, title, logoUrl, titled, detail, shownDetail, watchState } = card.backdrop;
  const verb = isStarted(watchState) ? "Resume" : "Play";
  return (
    <BackdropCard
      art={{ url, thumbhash, sizes: backdropTileSizes, widths: "backdropWidths" }}
      link={card.link}
      label={[`${verb} ${title}`, detail].filter(Boolean).join(", ")}
      caption={
        <ContinueWatchingCaption
          title={title}
          logoUrl={logoUrl}
          titled={titled}
          detail={shownDetail}
          state={watchState}
        />
      }
      menu={<CardMenu title={title} actions={actions} />}
    />
  );
}
