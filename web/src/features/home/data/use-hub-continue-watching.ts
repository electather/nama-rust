import { createQueryOptions } from "@connectrpc/connect-query";
import { useQueries } from "@tanstack/react-query";

import { useReachedServers } from "@/features/home/data/use-reached-servers";
import { entryCard } from "@/features/home/lib/continue-watching-cards";
import { cardOn, mergeContinueWatching } from "@/features/home/lib/hub-titles";
import { BrowseService } from "@/gen/nama/v1/browse_pb";
import { loaderStaleTime } from "@/shared/hooks/loader-stale-time";

// shortcut: only each Server's first page is merged; page through each when a shelf runs out.
const firstPage = { page: { size: 20 } };

/**
 * Continue Watching at the Hub: each Server's that has answered so far, as one, each title once,
 * from the Server it was last watched on.
 */
export function useHubContinueWatching() {
  const reached = useReachedServers();
  // Not suspended, as `useReachedServers` isn't.
  return useQueries({
    queries: reached.map(({ transport }) => ({
      ...createQueryOptions(BrowseService.method.listContinueWatching, firstPage, { transport }),
      staleTime: loaderStaleTime,
    })),
    combine: (answers) =>
      mergeContinueWatching(
        reached.map((on, at) =>
          (answers[at]?.data?.entries ?? [])
            .flatMap((entry) => entryCard(entry))
            .map((card) => cardOn(on, card)),
        ),
      ),
  });
}
