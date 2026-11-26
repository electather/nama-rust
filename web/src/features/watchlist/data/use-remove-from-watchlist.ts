import { callUnaryMethod } from "@connectrpc/connect-query";
import { useMutation, useQueryClient } from "@tanstack/react-query";

import type { Listing } from "@/features/watchlist/lib/merged-watchlist";
import { WatchlistService } from "@/gen/nama/v1/watchlist_pb";
import { watchlistKeys } from "@/shared/hooks/use-toggle-watchlist";

/** Takes a title off each Watchlist it's `listed` on, then refetches each of those Servers'. */
export function useRemoveFromWatchlist() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (listed: Listing[]) => {
      // Settled, not all: refetching while a slower Server's removal is still on its way would
      // show the title again.
      const removals = await Promise.allSettled(
        listed.map(async ({ transport, kind, id }) => {
          await callUnaryMethod(transport, WatchlistService.method.removeFromWatchlist, {
            kind,
            id,
          });
        }),
      );
      const refused = removals.find((removal) => removal.status === "rejected");
      if (refused !== undefined) {
        throw refused.reason;
      }
    },
    // Settled, not succeeded: one Server refusing still took it off the others.
    onSettled: async (_, __, listed) => {
      const keys = listed.flatMap(({ transport }) => watchlistKeys(transport));
      await Promise.all(
        keys.map(async (queryKey) => {
          await queryClient.invalidateQueries({ queryKey });
        }),
      );
    },
  });
}
