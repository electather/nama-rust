import { create } from "@bufbuild/protobuf";
import { callUnaryMethod, useTransport } from "@connectrpc/connect-query";
import { useSuspenseQuery } from "@tanstack/react-query";

import { useStopPlan } from "@/features/player/data/use-stop-plan";
import type { Asking } from "@/features/player/lib/asking";
import { cantPlayBecause } from "@/features/player/lib/cant-play-plan";
import { findDeviceDecoding } from "@/features/player/lib/device-decoding";
import { stopPlan } from "@/features/player/lib/plan-lifetime";
import { PlaybackPlanSchema, PlaybackService } from "@/gen/nama/v1/playback_pb";

/**
 * The plan for Version `versionId` as `asking` says, asked for once and never reused after, once
 * the Server is told the plan it replaces stopped, so that one's Transcode leaves room for it. A
 * Server too busy to Transcode it, or that fails to plan it, gives a plan that can't play, saying so.
 */
export function usePlan(versionId: bigint, asking: Asking) {
  const transport = useTransport();
  const tellStopped = useStopPlan();
  const { failedMethods, startPositionMs, audioTrack, playbackLimit, subtitleTrack, noSubtitles } =
    asking;
  const { replacing } = asking;
  return useSuspenseQuery({
    queryKey: [
      "planPlayback",
      String(versionId),
      failedMethods,
      String(startPositionMs),
      audioTrack,
      playbackLimit,
      subtitleTrack,
      noSubtitles,
      String(replacing),
    ],
    queryFn: async () => {
      if (replacing !== undefined) {
        await stopPlan(tellStopped, replacing);
      }
      const deviceDecoding = await findDeviceDecoding();
      const input = {
        versionId,
        deviceDecoding,
        failedMethods,
        startPositionMs,
        audioTrack,
        playbackLimit,
        subtitleTrack,
        noSubtitles,
      };
      try {
        const { plan } = await callUnaryMethod(
          transport,
          PlaybackService.method.planPlayback,
          input,
        );
        return create(PlaybackPlanSchema, plan);
      } catch (error) {
        return cantPlayBecause(error);
      }
    },
    staleTime: "static",
    gcTime: 0,
  }).data;
}
