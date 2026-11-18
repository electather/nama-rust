import { createQueryOptions, useTransport } from "@connectrpc/connect-query";
import { useSuspenseQueries } from "@tanstack/react-query";

import { foundTitles } from "@/features/action-bar/lib/found-titles";
import { BrowseService } from "@/gen/nama/v1/browse_pb";

/**
 * The Movies, Shows and Episodes the first page of a search for `query` finds, or none for a blank
 * `query`, which isn't sent.
 */
export function useFoundTitles(query: string) {
  const search = createQueryOptions(
    BrowseService.method.searchLibraries,
    { query, page: { token: "" } },
    { transport: useTransport() },
  );
  // A list of none or one query, since `useSuspenseQuery` can't be skipped.
  const queries: (typeof search)[] = query === "" ? [] : [search];
  const [found] = useSuspenseQueries({ queries });
  return found === undefined ? [] : foundTitles(found.data);
}
