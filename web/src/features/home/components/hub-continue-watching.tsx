import { useId } from "react";

import { ContinueWatchingFrame } from "@/features/home/components/continue-watching-frame";
import { HubContinueWatchingShelf } from "@/features/home/components/hub-continue-watching-shelf";
import { useHubCardActions } from "@/features/home/data/use-hub-card-actions";
import { useHubContinueWatching } from "@/features/home/data/use-hub-continue-watching";

/**
 * Continue Watching at the Hub, from every Server that has answered, its skeleton till one has,
 * naming each Server that failed. A card's menu acts on every Server that lists its title.
 */
export function HubContinueWatching() {
  const headingId = useId();
  const { leaving } = useHubCardActions();
  const { cards: all, failed, ...shelf } = useHubContinueWatching();
  const cards = all.filter(({ card }) => !leaving(card));
  if (cards.length === 0 && failed.length === 0 && !shelf.waiting) {
    return null;
  }
  return (
    <ContinueWatchingFrame headingId={headingId}>
      {failed.map((server) => (
        <p key={server.id} className="mb-4 text-muted-foreground">
          Couldn’t load Continue Watching from {server.name}.
        </p>
      ))}
      <HubContinueWatchingShelf cards={cards} {...shelf} />
    </ContinueWatchingFrame>
  );
}
