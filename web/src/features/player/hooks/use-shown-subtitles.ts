import type { RefObject } from "react";

import { useDrawnSubtitle } from "@/features/player/hooks/use-drawn-subtitle";
import type { usePlaybackPlan } from "@/features/player/hooks/use-playback-plan";
import { useSubtitles } from "@/features/player/hooks/use-subtitles";
import { SubtitleFormat } from "@/gen/nama/v1/playback_pb";

/**
 * Shows the plan's subtitle at the URL `shownSubtitle`, as a `<track>` or drawn, and says how each
 * shown so far is getting on. One drawn that fails to draw is chosen to burn in instead (ADR 0033).
 */
export function useShownSubtitles(
  {
    plan,
    shownSubtitle: shown,
    chooseSubtitle,
  }: Pick<ReturnType<typeof usePlaybackPlan>, "plan" | "shownSubtitle" | "chooseSubtitle">,
  videoRef: RefObject<HTMLVideoElement | null>,
) {
  const drawn = plan.subtitles.find(
    ({ url, format }) => url === shown && format !== SubtitleFormat.WEBVTT,
  );
  return {
    ...useSubtitles(videoRef, shown),
    ...useDrawnSubtitle(videoRef, drawn, () => {
      if (videoRef.current && drawn) {
        chooseSubtitle(videoRef.current, drawn.place);
      }
    }),
  };
}
