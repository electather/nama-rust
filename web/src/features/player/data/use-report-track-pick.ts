import { useMutation } from "@connectrpc/connect-query";

import { type PickedTrackKind, PlaybackService } from "@/gen/nama/v1/playback_pb";
import { retryIfUnreachable } from "@/shared/lib/connect-errors";

/**
 * Reports the Track of a kind the User picked while playing Version `versionId`, by its place in
 * the Version's Tracks, or `undefined` for subtitles off. Reports go out in the background, in
 * order, and a failed one is sent again, never shown.
 */
export function useReportTrackPick(versionId: bigint) {
  const { mutate } = useMutation(PlaybackService.method.reportTrackPick, {
    retry: retryIfUnreachable,
    // One at a time, so a retried pick never lands after a newer one.
    scope: { id: "report-track-pick" },
  });
  return (kind: PickedTrackKind, track: number | undefined) => {
    mutate({ versionId, kind, track });
  };
}
