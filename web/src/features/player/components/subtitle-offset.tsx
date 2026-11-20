import { OffsetControls } from "@/features/player/components/offset-controls";
import type { usePlaybackPlan } from "@/features/player/hooks/use-playback-plan";

/**
 * The shown subtitle's Subtitle Offset, which − and + shift at once. Disabled, saying why, while
 * one is burned in (ADR 0070), and left out while subtitles are off.
 */
export function SubtitleOffset({ planned }: { planned: ReturnType<typeof usePlaybackPlan> }) {
  const { plan, shownSubtitle, offsetOf, shiftSubtitle } = planned;
  const shown = plan.subtitles.find(({ url }) => url === shownSubtitle);
  if (shown !== undefined) {
    const offsetMs = offsetOf(shown);
    const shift = (by: number) => {
      shiftSubtitle(shown, offsetMs + by);
    };
    return <OffsetControls offsetMs={offsetMs} shift={shift} />;
  }
  return plan.subtitleTrack === undefined ? null : <OffsetControls offsetMs={0} />;
}
