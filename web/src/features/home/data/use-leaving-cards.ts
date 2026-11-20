import { useMutationState } from "@tanstack/react-query";

import { setHiddenKey } from "@/features/home/data/use-set-hidden-from-continue-watching";
import { askedText, removed, watched } from "@/features/home/lib/asked";
import type { Card } from "@/features/home/lib/continue-watching-cards";
import { setPlayedKey } from "@/shared/hooks/use-set-played";

/**
 * Whether a Continue Watching card is being marked watched or removed, so it leaves its shelf at
 * once. It stays gone till the Server answers and the shelf refetches, and comes back if the
 * Server refuses.
 */
export function useLeavingCards() {
  const asked = new Set([...usePending(setPlayedKey), ...usePending(setHiddenKey)]);
  return (card: Card) =>
    asked.has(askedText(watched(card.marking))) || asked.has(askedText(removed(card.hiding)));
}

/** What each pending mutation under `mutationKey` asks, as `askedText` writes it. */
function usePending(mutationKey: string[]) {
  return useMutationState({
    filters: { mutationKey, status: "pending" },
    select: ({ state }) => askedText(state.variables),
  });
}
