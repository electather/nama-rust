import { useMutation } from "@connectrpc/connect-query";

import { MatchService } from "@/gen/nama/v1/match_pb";
import { useMatchChanged } from "@/shared/hooks/use-match-changed";

/**
 * Matches an Unmatched Item, or every Item of a title, of the Library `libraryId` to a title, then
 * refetches its Unmatched Items and what browsing shows.
 */
export function useSetMatch(libraryId: bigint) {
  // Matching an Episode's Show may Unmatch its other Episodes, which no Event announces. Browsing
  // refetches too, so a player of the Item sees it Matched without waiting for the Event.
  return useMutation(MatchService.method.setMatch, { onSettled: useMatchChanged(libraryId) });
}
