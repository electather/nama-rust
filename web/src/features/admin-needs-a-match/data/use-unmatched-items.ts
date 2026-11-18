import type { Transport } from "@connectrpc/connect";

import type { Library } from "@/gen/nama/v1/library_pb";
import { MatchService } from "@/gen/nama/v1/match_pb";
import { pagedQuery, usePaged } from "@/shared/hooks/use-paged";

/** `ListUnmatchedItems`' first page of `library`'s Unmatched Items. */
function firstPageOf(library: Library) {
  return { filter: { libraryId: library.id }, page: { token: "" } };
}

/** `library`'s Unmatched Items, for the Needs a Match page's loader to load before it shows. */
export function unmatchedItemsQuery(library: Library, transport: Transport) {
  return {
    ...pagedQuery(MatchService.method.listUnmatchedItems, firstPageOf(library), transport),
    staleTime: "static" as const,
  };
}

/** `library`'s Unmatched Items, a page at a time. */
export function useUnmatchedItems(library: Library) {
  return usePaged(MatchService.method.listUnmatchedItems, firstPageOf(library));
}
