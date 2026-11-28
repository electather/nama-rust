import { useSuspenseQuery } from "@connectrpc/connect-query";
import { useMemo } from "react";

import { rowSeasons } from "@/features/title/lib/row-seasons";
import { BrowseService, type Season } from "@/gen/nama/v1/browse_pb";
import type { ExternalId } from "@/gen/nama/v1/match_pb";
import { loaderStaleTime } from "@/shared/hooks/loader-stale-time";

/**
 * Every Episode of the Show `show` in `seasons`, Season by Season in the row's order, the same
 * array while neither changes, so the Show's row keeps its identity.
 */
export function useShowEpisodes(
  show: { libraryId: bigint; showId: ExternalId },
  seasons: Season[],
) {
  const { data } = useSuspenseQuery(BrowseService.method.listEpisodes, show, {
    staleTime: loaderStaleTime,
  });
  return useMemo(
    () =>
      rowSeasons(seasons).flatMap((number) =>
        data.episodes.filter(({ seasonNumber }) => seasonNumber === number),
      ),
    [data.episodes, seasons],
  );
}
