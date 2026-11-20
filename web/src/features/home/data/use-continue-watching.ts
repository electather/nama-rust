import type { Transport } from "@connectrpc/connect";
import { createQueryOptions, useSuspenseQuery } from "@connectrpc/connect-query";

import { BrowseService } from "@/gen/nama/v1/browse_pb";
import { loaderStaleTime } from "@/shared/hooks/loader-stale-time";

/** Home's one page of Continue Watching: whole rows at one, two and three columns. */
const continueWatchingRow = { page: { size: 6 } };

/** Home's Continue Watching, for the route's loader to load before it shows. */
export function continueWatchingQuery(transport: Transport) {
  return {
    ...createQueryOptions(BrowseService.method.listContinueWatching, continueWatchingRow, {
      transport,
    }),
    staleTime: "static" as const,
  };
}

/** Home's Continue Watching entries, most recent first. */
export function useContinueWatching() {
  return useSuspenseQuery(BrowseService.method.listContinueWatching, continueWatchingRow, {
    staleTime: loaderStaleTime,
  }).data.entries;
}
