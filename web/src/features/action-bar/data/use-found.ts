import { createClient } from "@connectrpc/connect";
import { useTransport } from "@connectrpc/connect-query";
import {
  experimental_streamedQuery as streamedQuery,
  keepPreviousData,
  useQuery,
} from "@tanstack/react-query";

import { foundIn } from "@/features/action-bar/lib/titles";
import type { TitleKind } from "@/gen/nama/v1/match_pb";
import { type SearchResponse, SearchService } from "@/gen/nama/v1/search_pb";

/**
 * What a search of the signed-in User's Libraries for `words` of `kind` finds, and nothing for no
 * words. The last answer stays while the next one loads, and a new search aborts the last.
 */
export function useFound(words: string, kind: TitleKind) {
  const transport = useTransport();
  const asked = words !== "";
  // fallow-ignore-next-line policy-violation:feature-layout/suspense -- the list keeps its last results, and offers Retry as one of its rows, while it searches
  const query = useQuery({
    queryKey: ["search", words, kind],
    queryFn: streamedQuery<SearchResponse>({
      streamFn: ({ signal }) =>
        createClient(SearchService, transport).search({ query: words, kind }, { signal }),
    }),
    enabled: asked,
    placeholderData: keepPreviousData,
    staleTime: 30_000,
  });
  return {
    found: asked && query.data ? foundIn(query.data) : undefined,
    // A search that isn't asked for neither fetches nor fails.
    searching: query.isFetching,
    failed: query.isError,
    retry: () => {
      void query.refetch();
    },
  };
}
