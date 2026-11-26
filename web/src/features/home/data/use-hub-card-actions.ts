import { callUnaryMethod } from "@connectrpc/connect-query";
import { useMutation, useMutationState, useQueryClient } from "@tanstack/react-query";

import { removed, watched } from "@/features/home/lib/asked";
import type { Card } from "@/features/home/lib/continue-watching-cards";
import type { CardOn, MergedCard } from "@/features/home/lib/hub-titles";
import { WatchStateService } from "@/gen/nama/v1/watch_state_pb";
import { browseKey } from "@/shared/hooks/browse";

// Names no Server, unlike a key on one Server's data (`web/src/AGENTS.md`): one card's menu asks
// every Server that lists its title.
const hubCardKey = ["hubContinueWatchingCard"];

/** One Server's copy of a card at the Hub, marked watched or removed, under its title's id. */
interface Ask {
  titleId: string;
  copy: CardOn;
  to: "watch" | "remove";
}

/**
 * What a Continue Watching card's menu does at the Hub: `markWatched` a title, or `remove` it, on
 * every Server that lists it. Hiding doesn't sync, so a Server left out would bring it back. It's
 * `leaving` till each Server has answered and its shelf refetched.
 */
export function useHubCardActions() {
  const queryClient = useQueryClient();
  const { mutate } = useMutation({
    mutationKey: hubCardKey,
    mutationFn: async ({ copy: { transport, card }, to }: Ask) => {
      await (to === "watch"
        ? callUnaryMethod(transport, WatchStateService.method.setPlayed, watched(card.marking))
        : callUnaryMethod(
            transport,
            WatchStateService.method.setHiddenFromContinueWatching,
            removed(card.hiding),
          ));
    },
    onSettled: async (_answer, _error, { copy }) => {
      await queryClient.invalidateQueries({ queryKey: browseKey(copy.transport) });
    },
  });
  const asked = new Set(
    useMutationState({
      filters: { mutationKey: hubCardKey, status: "pending" },
      select: ({ state }) => titleOf(state.variables),
    }),
  );
  const ask = (to: Ask["to"]) => (merged: MergedCard) => {
    for (const copy of merged.copies) {
      mutate({ titleId: merged.card.titleId, copy, to });
    }
  };
  return {
    leaving: (card: Card) => asked.has(card.titleId),
    markWatched: ask("watch"),
    remove: ask("remove"),
  };
}

/** The title an `Ask` is about, as a pending mutation holds it, untyped. */
function titleOf(asked: unknown) {
  return typeof asked === "object" && asked !== null && "titleId" in asked
    ? asked.titleId
    : undefined;
}
