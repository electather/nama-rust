import { createQueryOptions, useTransport } from "@connectrpc/connect-query";
import { useSuspenseQueries } from "@tanstack/react-query";

import { BrowseService } from "@/gen/nama/v1/browse_pb";
import type { ExternalId } from "@/gen/nama/v1/match_pb";

/**
 * The Episodes of each of `seasons` of the Show `showId` names, in the same order.
 *
 * ponytail: one `ListEpisodes` per Season, all at once; a Show-wide RPC if a long Show's panel
 * opens slowly.
 */
export function useShowEpisodes(libraryId: bigint, showId: ExternalId, seasons: number[]) {
  const transport = useTransport();
  return useSuspenseQueries({
    queries: seasons.map((seasonNumber) =>
      createQueryOptions(
        BrowseService.method.listEpisodes,
        { libraryId, showId, seasonNumber },
        { transport },
      ),
    ),
  }).map(({ data }) => data.episodes);
}
