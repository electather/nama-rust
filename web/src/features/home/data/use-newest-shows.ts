import { create } from "@bufbuild/protobuf";

import type { ShelfTitles } from "@/features/home/lib/shelf-tiles";
import { BrowseService, ShowInLibrarySchema, ShowSort } from "@/gen/nama/v1/browse_pb";
import { SortDirection } from "@/gen/nama/v1/page_pb";
import { usePaged } from "@/shared/hooks/use-paged";
import type { RowTitle } from "@/shared/lib/row-titles";

/** As `useNewestMovies`'s page. */
const newestPage = { size: 20 };

/**
 * The Shows of the Shows Library `libraryId`, the one with the newest Episode first, page by page
 * as its shelf scrolls.
 */
export function useNewestShows(libraryId: bigint) {
  const paged = usePaged(BrowseService.method.listShows, {
    libraryId,
    sort: ShowSort.ADDED,
    direction: SortDirection.DESCENDING,
    page: newestPage,
  });
  const titles = paged.data.pages
    .flatMap((page) => page.shows)
    .map((show): RowTitle => ({
      case: "show",
      value: create(ShowInLibrarySchema, {
        libraryId,
        id: show.id,
        title: show.title,
        posterUrl: show.posterUrl,
        posterThumbhash: show.posterThumbhash,
      }),
    }));
  return {
    titles,
    setSize: paged.totalSize,
    next: { has: paged.hasNextPage, load: paged.fetchNextPage },
  } satisfies ShelfTitles;
}
