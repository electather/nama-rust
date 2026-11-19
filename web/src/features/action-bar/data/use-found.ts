import { useQuery } from "@connectrpc/connect-query";
import { keepPreviousData } from "@tanstack/react-query";

import { foundIn } from "@/features/action-bar/lib/titles";
import { BrowseService } from "@/gen/nama/v1/browse_pb";

/**
 * What a search of the signed-in User's Libraries for `words` finds, and nothing for no words. The
 * last answer stays while the next one loads.
 */
export function useFound(words: string) {
  const asked = words !== "";
  // ponytail: the first page only, which holds the best fits. Page on when a search needs more.
  // fallow-ignore-next-line policy-violation:feature-layout/suspense -- the list keeps its last results, and offers Retry as one of its rows, while it searches
  const query = useQuery(
    BrowseService.method.searchLibraries,
    { query: words, page: { token: "" } },
    { enabled: asked, placeholderData: keepPreviousData, staleTime: 30_000 },
  );
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
