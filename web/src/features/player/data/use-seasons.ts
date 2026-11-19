import { useSuspenseQuery } from "@connectrpc/connect-query";

import { BrowseService } from "@/gen/nama/v1/browse_pb";
import type { ExternalId } from "@/gen/nama/v1/match_pb";

/** The Seasons of the Show `showId` names in the Library `libraryId`. */
export function useSeasons(libraryId: bigint, showId: ExternalId) {
  return useSuspenseQuery(BrowseService.method.getShow, { libraryId, showId }).data.seasons;
}
