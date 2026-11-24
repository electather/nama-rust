import type { Transport } from "@connectrpc/connect";
import { createConnectQueryKey, useMutation, useTransport } from "@connectrpc/connect-query";
import { useQueryClient } from "@tanstack/react-query";

import type { ExternalId, TitleKind } from "@/gen/nama/v1/match_pb";
import { RequestService } from "@/gen/nama/v1/request_pb";
import { WatchlistService } from "@/gen/nama/v1/watchlist_pb";
import { browseKey } from "@/shared/hooks/browse";

/** What says whether a title is on your Watchlist: every browse query, and the request pages'. */
function watchlistKeys(transport: Transport) {
  const finite = { transport, cardinality: "finite" } as const;
  return [
    browseKey(transport),
    createConnectQueryKey({ schema: RequestService.method.lookUpMovie, ...finite }),
    createConnectQueryKey({ schema: RequestService.method.lookUpShow, ...finite }),
  ];
}

/** A title's place on your Watchlist, as `on_watchlist` says. */
export interface OnWatchlist {
  kind: TitleKind;
  id: ExternalId;
  on: boolean;
}

/**
 * Adds the title `watchlist` names to your Watchlist or takes it off, then refetches what's on
 * screen. It shows `on` as you asked at once, till the Server answers and the refetch is done, and
 * as it was if the Server refuses.
 */
export function useToggleWatchlist({ kind, id, on }: OnWatchlist) {
  const queryClient = useQueryClient();
  const transport = useTransport();
  const refetch = {
    onSuccess: async () => {
      await Promise.all(
        watchlistKeys(transport).map(async (queryKey) => {
          await queryClient.invalidateQueries({ queryKey });
        }),
      );
    },
  };
  const add = useMutation(WatchlistService.method.addToWatchlist, refetch);
  const remove = useMutation(WatchlistService.method.removeFromWatchlist, refetch);
  const pending = add.isPending || remove.isPending;
  const shown = pending ? add.isPending : on;
  return {
    on: shown,
    toggle: () => {
      // Pressed again before the Server answers, it would race the first ask.
      if (!pending) {
        (shown ? remove : add).mutate({ kind, id });
      }
    },
  };
}
