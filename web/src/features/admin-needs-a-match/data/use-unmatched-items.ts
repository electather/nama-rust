import type { Library } from "@/gen/nama/v1/library_pb";
import { MatchService } from "@/gen/nama/v1/match_pb";
import { usePaged } from "@/shared/hooks/use-paged";

/** `ListUnmatchedItems`' first page of `library`'s Unmatched Items. */
function firstPageOf(library: Library) {
  return { filter: { libraryId: library.id }, page: { token: "" } };
}

/** `library`'s Unmatched Items, a page at a time. */
export function useUnmatchedItems(library: Library) {
  return usePaged(MatchService.method.listUnmatchedItems, firstPageOf(library));
}
