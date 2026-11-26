import { useId } from "react";

import { ContinueWatchingFrame } from "@/features/home/components/continue-watching-frame";
import { OnServer } from "@/features/home/components/on-server";
import { ServerCard } from "@/features/home/components/server-card";
import { VirtualShelf } from "@/features/home/components/virtual-shelf";
import { useHubContinueWatching } from "@/features/home/data/use-hub-continue-watching";
import { backdropTile } from "@/features/home/lib/shelf-tiles";

/** Continue Watching at the Hub, from every Server that has answered, or nothing till one has. */
export function HubContinueWatching() {
  const headingId = useId();
  const cards = useHubContinueWatching();
  if (cards.length === 0) {
    return null;
  }
  const tiles = cards.map(({ server, transport, card }) => ({
    key: `${server.id}/${card.key}`,
    tile: (
      <OnServer transport={transport}>
        <ServerCard card={card} />
      </OnServer>
    ),
  }));
  return (
    <ContinueWatchingFrame headingId={headingId}>
      <VirtualShelf tiles={tiles} tile={backdropTile} setSize={tiles.length} />
    </ContinueWatchingFrame>
  );
}
