import type { Transport } from "@connectrpc/connect";
import { createQueryOptions, useSuspenseQuery, useTransport } from "@connectrpc/connect-query";

import { mergedWatchlist } from "@/features/watchlist/lib/merged-watchlist";
import { WatchlistService } from "@/gen/nama/v1/watchlist_pb";
import { loaderStaleTime } from "@/shared/hooks/loader-stale-time";

/** Your Watchlist on the Server `transport` reaches, for the Watchlist page's loader. */
export function watchlistQuery(transport: Transport) {
  return {
    ...createQueryOptions(WatchlistService.method.listWatchlist, {}, { transport }),
    staleTime: loaderStaleTime,
  };
}

/** Your Watchlist on this Server, each title once. */
export function useWatchlist() {
  const transport = useTransport();
  const { titles } = useSuspenseQuery(
    WatchlistService.method.listWatchlist,
    {},
    { staleTime: loaderStaleTime },
  ).data;
  return mergedWatchlist([{ transport, server: undefined, titles }]);
}
