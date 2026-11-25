import { createQueryOptions } from "@connectrpc/connect-query";
import { useQueries } from "@tanstack/react-query";

import { useLibrariesElsewhere } from "@/features/title/data/use-libraries-elsewhere";
import { onePerServer } from "@/features/title/lib/server-choices";
import { BrowseService, type Episode } from "@/gen/nama/v1/browse_pb";
import { LibraryKind } from "@/gen/nama/v1/library_pb";
import type { ExternalId } from "@/gen/nama/v1/match_pb";
import { loaderStaleTime } from "@/shared/hooks/loader-stale-time";

/**
 * At the Hub, `episode` of the Show `showId` on each of the Account's other Servers that has it,
 * found by its Season and number, since its own id there may be from another title source.
 */
export function useEpisodeElsewhere(showId: ExternalId, episode: Episode | undefined) {
  const libraries = useLibrariesElsewhere(LibraryKind.SHOWS);
  // Not suspended, as `useLibrariesElsewhere` isn't.
  return useQueries({
    // As the Show's page asks its own Server, so playing there finds it cached.
    queries: libraries.map(({ transport, libraryId }) => ({
      ...createQueryOptions(
        BrowseService.method.listEpisodes,
        { libraryId, showId },
        { transport },
      ),
      staleTime: loaderStaleTime,
    })),
    combine: (answers) =>
      onePerServer(
        libraries.flatMap(({ server, libraryId }, at) => {
          const found = answers[at]?.data?.episodes.find(
            ({ seasonNumber, episodeNumber }) =>
              seasonNumber === episode?.seasonNumber && episodeNumber === episode.episodeNumber,
          );
          return found
            ? [{ server, libraryId, versions: found.versions, watchState: found.watchState }]
            : [];
        }),
      ),
  });
}
