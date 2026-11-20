import { useQuery } from "@connectrpc/connect-query";

import { BrowseService } from "@/gen/nama/v1/browse_pb";
import { loaderStaleTime } from "@/shared/hooks/loader-stale-time";

/** The first page of your Continue Watching, most recent first. */
export function useContinueWatchingEntries() {
  const options = { staleTime: loaderStaleTime };
  // fallow-ignore-next-line policy-violation:feature-layout/suspense -- Resume is only an extra on the page, which shows without it while it loads or if it fails
  return useQuery(BrowseService.method.listContinueWatching, {}, options).data?.entries ?? [];
}
