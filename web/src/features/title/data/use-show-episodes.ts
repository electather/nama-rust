import { createQueryOptions, useTransport } from "@connectrpc/connect-query";
import { useSuspenseQueries } from "@tanstack/react-query";

import { BrowseService } from "@/gen/nama/v1/browse_pb";
import type { ExternalId } from "@/gen/nama/v1/match_pb";
import { loaderStaleTime } from "@/shared/hooks/loader-stale-time";

/** Every Episode of the Show `show` in `seasons`, Season by Season in their order. */
export function useShowEpisodes(
  show: { libraryId: bigint; showId: ExternalId },
  seasons: number[],
) {
  const transport = useTransport();
  const results = useSuspenseQueries({
    queries: seasons.map((seasonNumber) => ({
      ...createQueryOptions(
        BrowseService.method.listEpisodes,
        { ...show, seasonNumber },
        { transport },
      ),
      staleTime: loaderStaleTime,
    })),
  });
  return results.flatMap(({ data }) => data.episodes);
}
