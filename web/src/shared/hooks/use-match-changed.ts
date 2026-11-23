import { useQueryClient } from "@tanstack/react-query";

import { browseKey } from "@/shared/hooks/browse";
import { unmatchedKey } from "@/shared/hooks/unmatched-key";

/**
 * What to do once a Match of the Library `libraryId` changed: refetch its Unmatched Items and what
 * browsing shows.
 */
export function useMatchChanged(libraryId: bigint) {
  const queryClient = useQueryClient();
  return async () => {
    await Promise.all([
      queryClient.invalidateQueries({ queryKey: unmatchedKey(libraryId) }),
      queryClient.invalidateQueries({ queryKey: browseKey }),
    ]);
  };
}
