import { useMutation } from "@connectrpc/connect-query";
import { type QueryClient, useQueryClient } from "@tanstack/react-query";
import { useCallback } from "react";

import { type ReportProgressRequest, WatchStateService } from "@/gen/nama/v1/watch_state_pb";
import { browseKey } from "@/shared/hooks/browse";
import { retryIfUnreachable } from "@/shared/lib/connect-errors";

type Report = Pick<ReportProgressRequest, "versionId" | "positionMs" | "durationMs">;

// Marked on the report itself, as the player that sent a final one may already follow another
// Version or Episode, or be gone, by the time its answer arrives.
const finalReports = new WeakSet();

/**
 * Reports how far in the Movie or Episode you are, in the background, one at a time and in order,
 * and sends a failed report again. A `final` report, sent on leaving the player, refreshes
 * browsing once it lands; any other changes nothing the player shows, so refreshes nothing.
 */
export function useReportProgress() {
  const queryClient = useQueryClient();
  const { mutate } = useMutation(WatchStateService.method.reportProgress, {
    retry: retryIfUnreachable,
    // One at a time, in order, so a retried report never lands after a newer one.
    scope: { id: "report-progress" },
    onSuccess: async (_response, report) => {
      if (finalReports.has(report)) {
        await refreshWatchState(queryClient);
      }
    },
  });
  return useCallback(
    (report: Report, final: boolean) => {
      if (final) {
        finalReports.add(report);
      }
      mutate(report);
    },
    [mutate],
  );
}

/**
 * Refreshes what browsing shows after leaving the player. What's off screen is dropped, so opening
 * it again, such as reopening the player, fetches where you are now instead of starting from a
 * stale copy (the loaders never refetch what's cached), and what's on screen, such as the page Back
 * opened, refetches. ponytail: that drops every page you browsed, so going further back fetches it
 * again; keep only the played title's queries fresh if that cost shows.
 */
async function refreshWatchState(queryClient: QueryClient) {
  queryClient.removeQueries({ queryKey: browseKey, type: "inactive" });
  await queryClient.invalidateQueries({ queryKey: browseKey });
}
