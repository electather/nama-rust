import { firstPageOf, type TitleListing, unfiltered } from "@/features/library/lib/title-listing";
import { BrowseService } from "@/gen/nama/v1/browse_pb";
import { usePaged } from "@/shared/hooks/use-paged";

/**
 * How many Movies of the Library `libraryId` `listing` keeps, and how many it has in all. Each is
 * the grid's own first page, so neither asks the Server again; unfiltered, they're one.
 */
export function useMovieCounts(libraryId: bigint, listing: TitleListing) {
  const method = BrowseService.method.listMovies;
  const shown = usePaged(method, { libraryId, ...firstPageOf(listing, "movies") });
  const all = usePaged(method, { libraryId, ...firstPageOf(unfiltered(listing), "movies") });
  return { shown: shown.totalSize, all: all.totalSize };
}

/** As `useMovieCounts`, for a Shows Library's Shows. */
export function useShowCounts(libraryId: bigint, listing: TitleListing) {
  const method = BrowseService.method.listShows;
  const shown = usePaged(method, { libraryId, ...firstPageOf(listing, "shows") });
  const all = usePaged(method, { libraryId, ...firstPageOf(unfiltered(listing), "shows") });
  return { shown: shown.totalSize, all: all.totalSize };
}
