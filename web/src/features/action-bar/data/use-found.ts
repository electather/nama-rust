import { useTransport } from "@connectrpc/connect-query";
import { keepPreviousData, useQuery } from "@tanstack/react-query";

import { foundIn } from "@/features/action-bar/lib/titles";
import type { TitleKind } from "@/gen/nama/v1/match_pb";
import { searchQuery } from "@/shared/hooks/search-query";

/**
 * What a search for `words` of `kind` finds, in the signed-in User's Libraries and to request, and
 * nothing for no words. The last answer stays while the next one loads, and a new search aborts the last.
 */
export function useFound(words: string, kind: TitleKind) {
  const transport = useTransport();
  const asked = words !== "";
  // fallow-ignore-next-line policy-violation:feature-layout/suspense -- the list keeps its last results, and offers Retry as one of its rows, while it searches
  const query = useQuery({
    ...searchQuery(words, kind, transport),
    enabled: asked,
    placeholderData: keepPreviousData,
  });
  const found = asked && query.data ? foundIn(query.data) : undefined;
  return {
    found,
    note: found?.note,
    // A search that isn't asked for neither fetches nor fails.
    searching: query.isFetching,
    failed: query.isError,
    retry: () => {
      void query.refetch();
    },
  };
}
