import { useMutation } from "@connectrpc/connect-query";
import { useQueryClient } from "@tanstack/react-query";

import { MatchService } from "@/gen/nama/v1/match_pb";
import { browseKey } from "@/shared/hooks/browse";
import { unmatchedKey } from "@/shared/hooks/unmatched-key";

/**
 * Matches an Unmatched Item of the Library `libraryId` to a title, then refetches its Unmatched
 * Items and what browsing shows.
 */
export function useSetMatch(libraryId: bigint) {
  const queryClient = useQueryClient();
  // Matching an Episode's Show may Unmatch its other Episodes, which no Event announces. Browsing
  // refetches too, so a player of the Item sees it Matched without waiting for the Event.
  return useMutation(MatchService.method.setMatch, {
    onSuccess: async () => {
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: unmatchedKey(libraryId) }),
        queryClient.invalidateQueries({ queryKey: browseKey }),
      ]);
    },
  });
}
