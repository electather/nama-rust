import { useSuspenseQuery } from "@connectrpc/connect-query";

import { movieRowTitles, newestMovies } from "@/features/home/lib/newest-titles";
import { BrowseService } from "@/gen/nama/v1/browse_pb";
import { loaderStaleTime } from "@/shared/hooks/loader-stale-time";

/** The newest added Movies of the Movies Library `libraryId`, newest first. */
export function useNewestMovies(libraryId: bigint) {
  const { movies } = useSuspenseQuery(BrowseService.method.listMovies, newestMovies(libraryId), {
    staleTime: loaderStaleTime,
  }).data;
  return movieRowTitles(libraryId, movies);
}
