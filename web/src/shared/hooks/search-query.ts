import { type Transport, createClient } from "@connectrpc/connect";
import { experimental_streamedQuery as streamedQuery, queryOptions } from "@tanstack/react-query";

import type { TitleKind } from "@/gen/nama/v1/match_pb";
import { type SearchResponse, SearchService } from "@/gen/nama/v1/search_pb";
import { serverKey } from "@/shared/hooks/server-key";

/** Every search: what to refresh when a Request changes, since each title found carries its own. */
export function searchKey(transport: Transport) {
  return ["search", serverKey(transport)] as const;
}

/**
 * What `Search` streams for `words` of `kind`, as its parts arrive. A new search aborts the last.
 */
export function searchQuery(words: string, kind: TitleKind, transport: Transport) {
  return queryOptions({
    queryKey: [...searchKey(transport), words, kind],
    queryFn: streamedQuery<SearchResponse>({
      streamFn: ({ signal }) =>
        createClient(SearchService, transport).search({ query: words, kind }, { signal }),
      // A refresh after a Request changes keeps the titles listed until the new ones are all in.
      refetchMode: "replace",
    }),
    staleTime: 30_000,
  });
}
