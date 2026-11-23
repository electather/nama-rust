import { useSuspenseQuery } from "@connectrpc/connect-query";

import { BrowseService } from "@/gen/nama/v1/browse_pb";
import type { ExternalId } from "@/gen/nama/v1/match_pb";
import { loaderStaleTime } from "@/shared/hooks/loader-stale-time";

/** Every Episode of the Show `show` in `seasons`, Season by Season in their order. */
export function useShowEpisodes(
  show: { libraryId: bigint; showId: ExternalId },
  seasons: number[],
) {
  const { data } = useSuspenseQuery(BrowseService.method.listEpisodes, show, {
    staleTime: loaderStaleTime,
  });
  return seasons.flatMap((number) =>
    data.episodes.filter(({ seasonNumber }) => seasonNumber === number),
  );
}
