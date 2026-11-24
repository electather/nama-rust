import { useSuspenseQuery } from "@connectrpc/connect-query";

import { BrowseService } from "@/gen/nama/v1/browse_pb";
import type { ExternalId } from "@/gen/nama/v1/match_pb";
import { loaderStaleTime } from "@/shared/hooks/loader-stale-time";

/** The Episodes of each of `seasons` of the Show `showId` names, in the same order. */
export function useShowEpisodes(libraryId: bigint, showId: ExternalId, seasons: number[]) {
  const { data } = useSuspenseQuery(
    BrowseService.method.listEpisodes,
    { libraryId, showId },
    { staleTime: loaderStaleTime },
  );
  return seasons.map((number) =>
    data.episodes.filter(({ seasonNumber }) => seasonNumber === number),
  );
}
