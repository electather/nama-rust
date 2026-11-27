import type { Transport } from "@connectrpc/connect";
import { createQueryOptions, useSuspenseQuery, useTransport } from "@connectrpc/connect-query";

import { type PanelOn, featuredPanels } from "@/features/home/lib/featured-panels";
import { BrowseService } from "@/gen/nama/v1/browse_pb";
import { loaderStaleTime } from "@/shared/hooks/loader-stale-time";

/** Your Featured, for the route's loader to start before Home shows, without waiting on it. */
export function featuredQuery(transport: Transport) {
  return {
    ...createQueryOptions(BrowseService.method.listFeatured, {}, { transport }),
    staleTime: "static" as const,
  };
}

/** Your Featured's panels, heaviest first. */
export function useFeatured(): PanelOn[] {
  const transport = useTransport();
  const { titles } = useSuspenseQuery(
    BrowseService.method.listFeatured,
    {},
    {
      staleTime: loaderStaleTime,
    },
  ).data;
  return featuredPanels(titles).map((panel) => ({ server: undefined, transport, panel }));
}
