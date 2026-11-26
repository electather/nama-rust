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
      await Promise.all(
        listed.map(async ({ transport, kind, id }) => {
          await callUnaryMethod(transport, WatchlistService.method.removeFromWatchlist, {
            kind,
            id,
          });
        }),
      );
    },
    // Settled, not succeeded: one Server refusing still takes it off the others.
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
