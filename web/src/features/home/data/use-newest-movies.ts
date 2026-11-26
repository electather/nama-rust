import { create } from "@bufbuild/protobuf";
import { useSuspenseQuery } from "@connectrpc/connect-query";

import { recentlyAddedPage } from "@/features/home/lib/shelf-tiles";
import {
  BrowseService,
  type MovieSummary,
  MovieInLibrarySchema,
  MovieSort,
} from "@/gen/nama/v1/browse_pb";
import { SortDirection } from "@/gen/nama/v1/page_pb";
import { loaderStaleTime } from "@/shared/hooks/loader-stale-time";
import type { RowTitle } from "@/shared/lib/row-titles";

/** What `ListMovies` is asked for the newest added Movies of the Movies Library `libraryId`. */
export function newestMovies(libraryId: bigint) {
  return {
    libraryId,
    sort: MovieSort.ADDED,
    direction: SortDirection.DESCENDING,
    page: recentlyAddedPage,
  };
}

/** The Movies Library `libraryId`'s `movies` as a row's titles. */
export function movieRowTitles(libraryId: bigint, movies: MovieSummary[]) {
  return movies.map((movie): RowTitle => ({
    case: "movie",
    value: create(MovieInLibrarySchema, {
      libraryId,
      id: movie.id,
      title: movie.title,
      releaseDate: movie.releaseDate,
      posterUrl: movie.posterUrl,
      posterThumbhash: movie.posterThumbhash,
    }),
  }));
}

/** The newest added Movies of the Movies Library `libraryId`, newest first. */
export function useNewestMovies(libraryId: bigint) {
  const { movies } = useSuspenseQuery(BrowseService.method.listMovies, newestMovies(libraryId), {
    staleTime: loaderStaleTime,
  }).data;
  return movieRowTitles(libraryId, movies);
}
