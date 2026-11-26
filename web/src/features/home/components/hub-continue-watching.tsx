import { useId } from "react";

import { ContinueWatchingCard } from "@/features/home/components/continue-watching-card";
import { ContinueWatchingFrame } from "@/features/home/components/continue-watching-frame";
import { OnServer } from "@/features/home/components/on-server";
import { VirtualShelf } from "@/features/home/components/virtual-shelf";
import { useHubCardActions } from "@/features/home/data/use-hub-card-actions";
import { useHubContinueWatching } from "@/features/home/data/use-hub-continue-watching";
import { backdropTile } from "@/features/home/lib/shelf-tiles";

/**
 * Continue Watching at the Hub, from every Server that has answered, or nothing till one has. A
 * card's menu acts on every Server that lists its title.
 */
export function HubContinueWatching() {
  const headingId = useId();
  const { leaving, markWatched, remove } = useHubCardActions();
  const cards = useHubContinueWatching().filter(({ card }) => !leaving(card));
  if (cards.length === 0) {
    return null;
  }
  const tiles = cards.map((merged) => ({
    key: `${merged.server.id}/${merged.card.key}`,
    tile: (
      <OnServer transport={merged.transport}>
        <ContinueWatchingCard
          card={merged.card}
          actions={{
            page: merged.card.page,
            onMarkWatched: () => {
              markWatched(merged);
            },
            onRemove: () => {
              remove(merged);
            },
          }}
        />
      </OnServer>
    ),
  }));
  return (
    <ContinueWatchingFrame headingId={headingId}>
      <VirtualShelf tiles={tiles} tile={backdropTile} setSize={tiles.length} />
    </ContinueWatchingFrame>
  );
}
