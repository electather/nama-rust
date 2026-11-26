import { create } from "@bufbuild/protobuf";

import { recentlyAddedPage } from "@/features/home/lib/shelf-tiles";
import {
  type MovieSummary,
  MovieInLibrarySchema,
  MovieSort,
  ShowInLibrarySchema,
  type ShowSummary,
  ShowSort,
} from "@/gen/nama/v1/browse_pb";
import { SortDirection } from "@/gen/nama/v1/page_pb";
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

/** What `ListShows` is asked for the Shows of the Shows Library `libraryId` added to last. */
export function newestShows(libraryId: bigint) {
  return {
    libraryId,
    sort: ShowSort.ADDED,
    direction: SortDirection.DESCENDING,
    page: recentlyAddedPage,
  };
}

/** The Shows Library `libraryId`'s `shows` as a row's titles. */
export function showRowTitles(libraryId: bigint, shows: ShowSummary[]) {
  return shows.map((show): RowTitle => ({
    case: "show",
    value: create(ShowInLibrarySchema, {
      libraryId,
      id: show.id,
      title: show.title,
      posterUrl: show.posterUrl,
      posterThumbhash: show.posterThumbhash,
    }),
  }));
}
