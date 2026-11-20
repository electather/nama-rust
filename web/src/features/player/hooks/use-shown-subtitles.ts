import type { RefObject } from "react";

import { useCueOffset } from "@/features/player/hooks/use-cue-offset";
import { useDrawnSubtitle } from "@/features/player/hooks/use-drawn-subtitle";
import type { usePlaybackPlan } from "@/features/player/hooks/use-playback-plan";
import { useSubtitles } from "@/features/player/hooks/use-subtitles";
import { SubtitleFormat } from "@/gen/nama/v1/playback_pb";

/**
 * Shows the plan's subtitle at the URL `shownSubtitle`, as a `<track>` or drawn, shifted by its
 * Subtitle Offset, and says how each shown so far is getting on. One drawn that fails to draw is chosen to burn in instead (ADR 0033).
 */
export function useShownSubtitles(
  {
    plan,
    shownSubtitle: shown,
    chooseSubtitle,
    offsetOf,
  }: Pick<
    ReturnType<typeof usePlaybackPlan>,
    "plan" | "shownSubtitle" | "chooseSubtitle" | "offsetOf"
  >,
  videoRef: RefObject<HTMLVideoElement | null>,
) {
  const shownOne = plan.subtitles.find(({ url }) => url === shown);
  const offsetMs = shownOne === undefined ? 0 : offsetOf(shownOne);
  const drawn = shownOne?.format === SubtitleFormat.WEBVTT ? undefined : shownOne;
  useCueOffset(videoRef, shown, offsetMs);
  return {
    ...useSubtitles(videoRef, shown),
    ...useDrawnSubtitle(videoRef, { subtitle: drawn, offsetMs }, () => {
      if (videoRef.current && drawn) {
        chooseSubtitle(videoRef.current, drawn.place);
      }
    }),
  };
}
