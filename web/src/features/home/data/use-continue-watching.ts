import type { Transport } from "@connectrpc/connect";

import { BrowseService } from "@/gen/nama/v1/browse_pb";
import { pagedQuery, usePaged } from "@/shared/hooks/use-paged";

/** A page of Continue Watching: a little over a wide screen's shelf of cards. */
const continueWatchingPage = { page: { size: 8 } };

/** Home's Continue Watching, for the route's loader to start before Home shows. */
export function continueWatchingQuery(transport: Transport) {
  return {
    ...pagedQuery(BrowseService.method.listContinueWatching, continueWatchingPage, transport),
    staleTime: "static" as const,
  };
}

/** Your Continue Watching, most recent first, page by page as its shelf scrolls. */
export function useContinueWatching() {
  return usePaged(BrowseService.method.listContinueWatching, continueWatchingPage);
}
