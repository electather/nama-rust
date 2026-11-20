import { useLeavingCards } from "@/features/home/data/use-leaving-cards";
import { useSetHiddenFromContinueWatching } from "@/features/home/data/use-set-hidden-from-continue-watching";
import { removed, watched } from "@/features/home/lib/asked";
import type { Card } from "@/features/home/lib/continue-watching-cards";
import { useSetPlayed } from "@/shared/hooks/use-set-played";

/**
 * What a Continue Watching card's menu does: `markWatched` a card, or `remove` one, either of
 * which it's `leaving` till it's done.
 */
export function useCardActions() {
  const setHidden = useSetHiddenFromContinueWatching();
  const setPlayed = useSetPlayed();
  return {
    leaving: useLeavingCards(),
    markWatched: (card: Card) => {
      setPlayed(watched(card.marking));
    },
    remove: (card: Card) => {
      setHidden(removed(card.hiding));
    },
  };
}
