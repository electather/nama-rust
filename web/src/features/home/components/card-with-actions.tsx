import { ContinueWatchingCard } from "@/features/home/components/continue-watching-card";
import { useCardActions } from "@/features/home/hooks/use-card-actions";
import type { Card } from "@/features/home/lib/continue-watching-cards";

/**
 * A Continue Watching card whose menu marks it watched or removes it on the Server the page
 * talks to, gone while it does.
 */
export function CardWithActions({ card }: { card: Card }) {
  const { leaving, markWatched, remove } = useCardActions();
  if (leaving(card)) {
    return null;
  }
  const actions = {
    page: card.page,
    onMarkWatched: () => {
      markWatched(card);
    },
    onRemove: () => {
      remove(card);
    },
  };
  return <ContinueWatchingCard card={card} actions={actions} />;
}
