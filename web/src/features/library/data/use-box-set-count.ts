import { firstBoxSetPage } from "@/features/library/lib/library-view";
import { BrowseService } from "@/gen/nama/v1/browse_pb";
import { usePaged } from "@/shared/hooks/use-paged";

/** How many Box Sets the Movies Library `libraryId` has, from the Box Sets tab's own first page. */
export function useBoxSetCount(libraryId: bigint) {
  return usePaged(BrowseService.method.listBoxSets, firstBoxSetPage(libraryId)).totalSize;
}
