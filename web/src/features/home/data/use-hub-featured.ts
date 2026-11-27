import { createQueryOptions } from "@connectrpc/connect-query";
import { useQueries } from "@tanstack/react-query";

import { featuredPanels } from "@/features/home/lib/featured-panels";
import { mergeFeatured, panelOn } from "@/features/home/lib/hub-titles";
import { useReachedServers } from "@/features/servers/data/use-reached-servers";
import { BrowseService } from "@/gen/nama/v1/browse_pb";
import { loaderStaleTime } from "@/shared/hooks/loader-stale-time";
import { useLinkedServers } from "@/shared/hooks/use-linked-servers";

/** Featured at the Hub, each Server's in turn, from the Servers that have answered so far. */
export function useHubFeatured() {
  const { reached } = useReachedServers(useLinkedServers());
  // Not suspended, so a slow or failing Server holds up neither Featured nor the rest.
  return useQueries({
    queries: reached.map(({ transport }) => ({
      ...createQueryOptions(BrowseService.method.listFeatured, {}, { transport }),
      staleTime: loaderStaleTime,
    })),
    combine: (answers) =>
      mergeFeatured(
        reached.map((server, at) =>
          featuredPanels(answers[at]?.data?.titles ?? []).map((panel) => panelOn(server, panel)),
        ),
      ),
  });
}
