import { createQueryOptions } from "@connectrpc/connect-query";
import { useQueries } from "@tanstack/react-query";

import { useLibrariesElsewhere } from "@/features/title/data/use-libraries-elsewhere";
import { onePerServer } from "@/features/title/lib/copies";
import { BrowseService } from "@/gen/nama/v1/browse_pb";
import { LibraryKind } from "@/gen/nama/v1/library_pb";
import type { ExternalId } from "@/gen/nama/v1/match_pb";
import { loaderStaleTime } from "@/shared/hooks/loader-stale-time";

/** At the Hub, the Movie `movieId` on each of the Account's other Servers that has it. */
export function useMovieElsewhere(movieId: ExternalId) {
  const libraries = useLibrariesElsewhere(LibraryKind.MOVIES);
  // Each Movies Library is asked, as a Movie is found only by its Library and its External ID; a
  // Library without it answers `not_found`.
  return useQueries({
    queries: libraries.map(({ transport, libraryId }) => ({
      ...createQueryOptions(BrowseService.method.getMovie, { libraryId, movieId }, { transport }),
      staleTime: loaderStaleTime,
    })),
    combine: (answers) =>
      onePerServer(
        libraries.flatMap(({ server, libraryId }, at) => {
          const movie = answers[at]?.data?.movie;
          return movie
            ? [{ server, libraryId, versions: movie.versions, watchState: movie.watchState }]
            : [];
        }),
      ),
  });
}
