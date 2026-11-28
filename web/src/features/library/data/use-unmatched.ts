import { firstUnmatchedPage } from "@/features/library/lib/first-unmatched-page";
import { BrowseService } from "@/gen/nama/v1/browse_pb";
import { usePaged } from "@/shared/hooks/use-paged";

/**
 * The pages of the Library `libraryId`'s Unmatched Items so far. Its input is the route loader's
 * prefetch (`firstUnmatchedPage`), so the loader's pages are these.
 */
export function useUnmatched(libraryId: bigint) {
  return usePaged(BrowseService.method.listUnmatched, firstUnmatchedPage(libraryId));
}
