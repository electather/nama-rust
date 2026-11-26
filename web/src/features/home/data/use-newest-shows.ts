import { useSuspenseQuery } from "@connectrpc/connect-query";

import { newestShows, showRowTitles } from "@/features/home/lib/newest-titles";
import { BrowseService } from "@/gen/nama/v1/browse_pb";
import { loaderStaleTime } from "@/shared/hooks/loader-stale-time";

/** The Shows of the Shows Library `libraryId` with the newest added Episodes, newest first. */
export function useNewestShows(libraryId: bigint) {
  const { shows } = useSuspenseQuery(BrowseService.method.listShows, newestShows(libraryId), {
    staleTime: loaderStaleTime,
  }).data;
  return showRowTitles(libraryId, shows);
}
