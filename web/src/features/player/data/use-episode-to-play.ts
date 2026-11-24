import { useSuspenseQuery } from "@connectrpc/connect-query";

import type { PlayedSeason } from "@/features/player/lib/played-season";
import { BrowseService } from "@/gen/nama/v1/browse_pb";
import { loaderStaleTime } from "@/shared/hooks/loader-stale-time";

/**
 * Episode `episodeNumber` of the played Season, unless an Event removed it, and the error of the
 * last refetch, if it failed.
 */
export function useEpisodeToPlay(
  { libraryId, showId, seasonNumber }: PlayedSeason,
  episodeNumber: number,
) {
  // The whole Show's, the Show page's cache entry, so Play from it asks for no more.
  const { data, error } = useSuspenseQuery(
    BrowseService.method.listEpisodes,
    { libraryId, showId },
    { staleTime: loaderStaleTime },
  );
  const episode = data.episodes.find(
    (each) => each.seasonNumber === seasonNumber && each.episodeNumber === episodeNumber,
  );
  return { episode, error };
}
