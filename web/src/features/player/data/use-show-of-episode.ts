import { useQuery } from "@connectrpc/connect-query";

import { BrowseService } from "@/gen/nama/v1/browse_pb";
import type { ExternalId } from "@/gen/nama/v1/match_pb";

/** The Show of the Library `libraryId` that `showId` names, once it arrives. */
export function useShowOfEpisode(libraryId: bigint, showId: ExternalId) {
  // fallow-ignore-next-line policy-violation:feature-layout/suspense -- the Episode doesn't wait for its Show
  return useQuery(BrowseService.method.getShow, { libraryId, showId }).data?.show;
}
