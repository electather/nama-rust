import { useMutation } from "@connectrpc/connect-query";
import { useQueryClient } from "@tanstack/react-query";

import type { ExternalId, TitleKind } from "@/gen/nama/v1/match_pb";
import { WatchlistService } from "@/gen/nama/v1/watchlist_pb";
import { browseKey } from "@/shared/hooks/browse";

/** A title's place on your Watchlist, as `on_watchlist` says. */
export interface OnWatchlist {
  kind: TitleKind;
  id: ExternalId;
  on: boolean;
}

/**
 * Adds a title to your Watchlist or takes it off, then refetches what's on screen, so its button
 * shows the Server's word. `pending` while it's asking.
 */
export function useToggleWatchlist() {
  const queryClient = useQueryClient();
  const refetch = {
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: browseKey });
    },
  };
  const add = useMutation(WatchlistService.method.addToWatchlist, refetch);
  const remove = useMutation(WatchlistService.method.removeFromWatchlist, refetch);
  return {
    pending: add.isPending || remove.isPending,
    toggle: ({ kind, id, on }: OnWatchlist) => {
      (on ? remove : add).mutate({ kind, id });
    },
  };
}
