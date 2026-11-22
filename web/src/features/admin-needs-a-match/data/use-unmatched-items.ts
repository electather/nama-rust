import { useTransport } from "@connectrpc/connect-query";
import { useQueryClient } from "@tanstack/react-query";

import type { Library } from "@/gen/nama/v1/library_pb";
import { MatchService } from "@/gen/nama/v1/match_pb";
import { pagedQuery, usePaged } from "@/shared/hooks/use-paged";

/** `ListUnmatchedItems`' first page of `library`'s Unmatched Items. */
function firstPageOf(library: Library) {
  return { filter: { libraryId: library.id }, page: { token: "" } };
}

/** `library`'s Unmatched Items, a page at a time. */
export function useUnmatchedItems(library: Library) {
  return usePaged(MatchService.method.listUnmatchedItems, firstPageOf(library));
}

/**
 * Starts loading the first page of each of `libraries`' Unmatched Items at once. Each Library's
 * list suspends only once the one before it has loaded, so without this they'd load one by one.
 */
export function usePrefetchUnmatchedItems(libraries: Library[]) {
  const queryClient = useQueryClient();
  const transport = useTransport();
  // During render, as TanStack's `usePrefetchInfiniteQuery` does: the lists suspend on these.
  for (const library of libraries) {
    const query = pagedQuery(
      MatchService.method.listUnmatchedItems,
      firstPageOf(library),
      transport,
    );
    if (queryClient.getQueryState(query.queryKey) === undefined) {
      void (async () => {
        try {
          await queryClient.infiniteQuery(query);
        } catch {
          // Its list asks again and shows why.
        }
      })();
    }
  }
}
