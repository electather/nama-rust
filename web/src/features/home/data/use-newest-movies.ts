import { create } from "@bufbuild/protobuf";

import type { ShelfTitles } from "@/features/home/lib/shelf-tiles";
import { BrowseService, MovieInLibrarySchema, MovieSort } from "@/gen/nama/v1/browse_pb";
import { SortDirection } from "@/gen/nama/v1/page_pb";
import { usePaged } from "@/shared/hooks/use-paged";
import type { RowTitle } from "@/shared/lib/row-titles";

/** A page of a Recently Added row: about 20 posters, a little over two wide screens' worth. */
const newestPage = { size: 20 };

/** The Movies of the Movies Library `libraryId`, newest added first, page by page as its shelf scrolls. */
export function useNewestMovies(libraryId: bigint) {
  const paged = usePaged(BrowseService.method.listMovies, {
    libraryId,
    sort: MovieSort.ADDED,
    direction: SortDirection.DESCENDING,
    page: newestPage,
  });
  const titles = paged.data.pages
    .flatMap((page) => page.movies)
    .map((movie): RowTitle => ({
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
  return {
    titles,
    setSize: paged.totalSize,
    next: { has: paged.hasNextPage, load: paged.fetchNextPage },
  } satisfies ShelfTitles;
}
