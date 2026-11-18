import { callUnaryMethod, useTransport } from "@connectrpc/connect-query";
import { useCallback } from "react";

import { PlaybackService } from "@/gen/nama/v1/playback_pb";

/** Tells the Server the plan `planId` stopped. A failed stop is left to the minute. */
export function useStopPlan() {
  const transport = useTransport();
  return useCallback(
    async (planId: bigint) => {
      try {
        await callUnaryMethod(transport, PlaybackService.method.stopPlayback, { planId });
      } catch {
        // Left to the minute.
      }
    },
    [transport],
  );
}
