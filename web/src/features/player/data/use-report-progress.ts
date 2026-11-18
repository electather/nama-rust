import { useMutation } from "@connectrpc/connect-query";
import { type QueryClient, useQueryClient } from "@tanstack/react-query";

import { WatchStateService } from "@/gen/nama/v1/watch_state_pb";
import { browseKey } from "@/lib/browse";
import { retryIfUnreachable } from "@/shared/lib/connect-errors";

/**
 * Reports how far in the Movie or Episode you are, in the background, one at a time and in order,
 * and sends a failed report again.
 */
export function useReportProgress() {
  const queryClient = useQueryClient();
  const { mutate } = useMutation(WatchStateService.method.reportProgress, {
    retry: retryIfUnreachable,
    // One at a time, in order, so a retried report never lands after a newer one.
    scope: { id: "report-progress" },
    onSuccess: async () => {
      await refreshWatchState(queryClient);
    },
  });
  return mutate;
}

/**
 * Refreshes what browsing shows after a report. What's off screen is dropped, so opening it again,
 * such as reopening the player, fetches where you are now instead of starting from a stale copy
 * (the loaders never refetch what's cached). ponytail: that drops every page you browsed, so Back
 * fetches it again, and the player's own title refetches after each report; keep only the played
 * title's queries fresh if that ever shows.
 */
async function refreshWatchState(queryClient: QueryClient) {
  queryClient.removeQueries({ queryKey: browseKey, type: "inactive" });
  await queryClient.invalidateQueries({ queryKey: browseKey });
}
