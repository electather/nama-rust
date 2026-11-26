import { createQueryOptions } from "@connectrpc/connect-query";
import { useQueries } from "@tanstack/react-query";

import { movieRowTitles, newestMovies } from "@/features/home/data/use-newest-movies";
import { newestShows, showRowTitles } from "@/features/home/data/use-newest-shows";
import type { RowOn } from "@/features/home/lib/hub-rows";
import { mergeTitles } from "@/features/home/lib/hub-titles";
import { BrowseService, type RecentlyAddedHomeRow } from "@/gen/nama/v1/browse_pb";
import { LibraryKind } from "@/gen/nama/v1/library_pb";
import { loaderStaleTime } from "@/shared/hooks/loader-stale-time";

/**
 * The newest titles of each Library whose Recently Added row is one of `sources`, all of one
 * kind, as one row, as each Server answers.
 */
export function useHubNewest(sources: RowOn<RecentlyAddedHomeRow>[]) {
  const shows = sources.filter(({ row }) => row.libraryKind === LibraryKind.SHOWS);
  const movies = sources.filter(({ row }) => row.libraryKind !== LibraryKind.SHOWS);
  // Not suspended, so a slow Server holds up neither the row nor the rest.
  const movieLists = useQueries({
    queries: movies.map(({ transport, row }) => ({
      ...createQueryOptions(BrowseService.method.listMovies, newestMovies(row.libraryId), {
        transport,
      }),
      staleTime: loaderStaleTime,
    })),
    combine: (answers) =>
      movies.map(({ server, transport, row }, at) =>
        movieRowTitles(row.libraryId, answers[at]?.data?.movies ?? []).map((title) => ({
          server,
          transport,
          title,
        })),
      ),
  });
  const showLists = useQueries({
    queries: shows.map(({ transport, row }) => ({
      ...createQueryOptions(BrowseService.method.listShows, newestShows(row.libraryId), {
        transport,
      }),
      staleTime: loaderStaleTime,
    })),
    combine: (answers) =>
      shows.map(({ server, transport, row }, at) =>
        showRowTitles(row.libraryId, answers[at]?.data?.shows ?? []).map((title) => ({
          server,
          transport,
          title,
        })),
      ),
  });
  return mergeTitles([...movieLists, ...showLists]);
}
