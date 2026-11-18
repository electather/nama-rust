import type { Transport } from "@connectrpc/connect";

import { BrowseService } from "@/gen/nama/v1/browse_pb";
import { pagedQuery, usePaged } from "@/shared/hooks/use-paged";

/** `SearchLibraries`' first page of what a search for `query` finds. */
function firstPageOf(query: string) {
  return { query, page: { token: "" } };
}

/** The first page of what a search for `query` finds, for the search route's loader to load before it shows. */
export function searchQuery(query: string, transport: Transport) {
  return {
    ...pagedQuery(BrowseService.method.searchLibraries, firstPageOf(query), transport),
    staleTime: "static" as const,
  };
}

/** What a search for `query` found, a page at a time. */
export function useSearchResults(query: string) {
  return usePaged(BrowseService.method.searchLibraries, firstPageOf(query));
}
