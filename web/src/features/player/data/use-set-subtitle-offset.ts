import { useMutation } from "@connectrpc/connect-query";

import { PlaybackService } from "@/gen/nama/v1/playback_pb";
import { retryIfUnreachable } from "@/shared/lib/connect-errors";

/**
 * Saves the User's Subtitle Offset for the subtitle Track at a place in Version `versionId`'s
 * Tracks. Saves go out in the background, in order, and a failed one is sent again, never shown.
 */
export function useSetSubtitleOffset(versionId: bigint) {
  const { mutate } = useMutation(PlaybackService.method.setSubtitleOffset, {
    retry: retryIfUnreachable,
    // One at a time, so a retried save never lands after a newer one.
    scope: { id: "set-subtitle-offset" },
  });
  return (track: number, offsetMs: number) => {
    mutate({ versionId, track, offsetMs });
  };
}
