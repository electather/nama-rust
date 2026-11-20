import type { Transport } from "@connectrpc/connect";
import { createQueryOptions, useSuspenseQuery } from "@connectrpc/connect-query";

import { BrowseService } from "@/gen/nama/v1/browse_pb";
import { loaderStaleTime } from "@/shared/hooks/loader-stale-time";

/** Your Featured, for the route's loader to start before Home shows, without waiting on it. */
export function featuredQuery(transport: Transport) {
  return {
    ...createQueryOptions(BrowseService.method.listFeatured, {}, { transport }),
    staleTime: "static" as const,
  };
}

/** Your Featured titles, heaviest first. */
export function useFeatured() {
  return useSuspenseQuery(BrowseService.method.listFeatured, {}, { staleTime: loaderStaleTime })
    .data.titles;
}
