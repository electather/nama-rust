import { useMutation } from "@connectrpc/connect-query";

import { MatchService } from "@/gen/nama/v1/match_pb";
import { useMatchChanged } from "@/shared/hooks/use-match-changed";

/**
 * Unmatches every Item of a Movie or Show of the Library `libraryId`, then refetches its Unmatched
 * Items and what browsing shows, where the title is gone.
 */
export function useClearTitleMatch(libraryId: bigint) {
  return useMutation(MatchService.method.clearMatch, { onSettled: useMatchChanged(libraryId) });
}
