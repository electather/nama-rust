import { useQueryClient } from "@tanstack/react-query";

import { browseKey } from "@/shared/hooks/browse";
import { unmatchedKey } from "@/shared/hooks/unmatched-key";

/**
 * What to do once a Match of the Library `libraryId` may have changed, failed calls included, since
 * a title's names are Matched one by one: refetch its Unmatched Items and what browsing shows.
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
