import { create } from "@bufbuild/protobuf";
import { callUnaryMethod, useTransport } from "@connectrpc/connect-query";
import { useSuspenseQuery } from "@tanstack/react-query";

import { planKey, usePlanLifetime } from "@/features/player/data/use-plan-lifetime";
import type { Asking } from "@/features/player/lib/asking";
import { cantPlayBecause } from "@/features/player/lib/cant-play-plan";
import { findDeviceDecoding } from "@/features/player/lib/device-decoding";
import {
  PlanPlaybackResponseSchema,
  PlaybackPlanSchema,
  PlaybackService,
} from "@/gen/nama/v1/playback_pb";
import { adminStopQuery } from "@/shared/hooks/admin-stop-query";

/**
 * The plan for Version `versionId` as `asking` says, asked for once and never reused after, once
 * the Server is told the plan it replaces stopped, so that one's Transcode leaves room for it. Its
 * player forgets it on closing (`usePlanLifetime`). A Server too busy to Transcode it, or that
 * fails to plan it, gives a plan that can't play, saying so. With it come the title's other
 * Versions that play on this Device, by id, best first, when the plan can't play.
 */
export function usePlan(versionId: bigint, asking: Asking) {
  const transport = useTransport();
  const { stopReplaced } = usePlanLifetime();
  const { failedRecipes, startPositionMs, audioTrack, playbackLimit, subtitleTrack, noSubtitles } =
    asking;
  const { replacing } = asking;
  const { plan, playableVersions } = useSuspenseQuery({
    queryKey: [
      ...planKey(transport),
      String(versionId),
      failedRecipes,
      String(startPositionMs),
      audioTrack,
      playbackLimit,
      subtitleTrack,
      noSubtitles,
      String(replacing),
    ],
    queryFn: async ({ client }) => {
      if (replacing !== undefined) {
        await stopReplaced(replacing);
      }
      const deviceDecoding = await findDeviceDecoding();
      const input = {
        versionId,
        deviceDecoding,
        failedRecipes,
        startPositionMs,
        audioTrack,
        playbackLimit,
        subtitleTrack,
        noSubtitles,
      };
      try {
        const response = await callUnaryMethod(
          transport,
          PlaybackService.method.planPlayback,
          input,
        );
        const planned = create(PlaybackPlanSchema, response.plan);
        // Plan ids start again when the Server restarts, so a stop kept under this one was another's.
        client.removeQueries(adminStopQuery(transport, planned.id));
        return create(PlanPlaybackResponseSchema, {
          plan: planned,
          playableVersions: response.playableVersions,
        });
      } catch (error) {
        return create(PlanPlaybackResponseSchema, { plan: cantPlayBecause(error) });
      }
    },
    staleTime: "static",
    // Suspense raises it to 1 s, so `usePlanLifetime` drops the plan.
    gcTime: 0,
  }).data;
  // Always set above: a message field is optional only in its type.
  return { plan: plan ?? create(PlaybackPlanSchema), playableVersions };
}
