import { Captions } from "lucide-react";
import type { RefObject } from "react";

import { SubtitleChoices } from "@/features/player/components/subtitle-choices";
import { useReportTrackPick } from "@/features/player/data/use-report-track-pick";
import type { usePlaybackPlan } from "@/features/player/hooks/use-playback-plan";
import { useShownSubtitles } from "@/features/player/hooks/use-shown-subtitles";
import type { Track } from "@/gen/nama/v1/browse_pb";
import { Button } from "@/shared/ui/button";
import { DropdownMenu, DropdownMenuContent, DropdownMenuTrigger } from "@/shared/ui/dropdown-menu";

/**
 * Picks the subtitle shown, or none: one of the plan's subtitles, which the Device shows itself,
 * saying which is still being read out of the file, or one of the Version's `tracks` the plan
 * offers to burn into the picture, an image one it can't draw or an ASS one it doesn't draw, with
 * its styling. Each pick is reported, so the Server keeps it for the Show or Movie the Version is
 * of. Absent when there's none to pick.
 */
export function SubtitlesMenu({
  tracks,
  planned,
  videoRef,
  container,
}: {
  tracks: Track[];
  planned: ReturnType<typeof usePlaybackPlan>;
  videoRef: RefObject<HTMLVideoElement | null>;
  /** Where the menu opens: inside the player, so it shows in full screen. */
  container: RefObject<HTMLDivElement | null>;
}) {
  const { versionId, plan } = planned;
  const { subtitles, burnInTracks: burnable } = plan;
  const states = useShownSubtitles(planned, videoRef);
  const reportPick = useReportTrackPick(versionId);
  if (subtitles.length + burnable.length === 0) {
    return null;
  }
  return (
    <DropdownMenu>
      <DropdownMenuTrigger render={<Button variant="overlay" size="icon" aria-label="Subtitles" />}>
        <Captions aria-hidden />
      </DropdownMenuTrigger>
      <DropdownMenuContent container={container}>
        <SubtitleChoices
          tracks={tracks}
          planned={planned}
          videoRef={videoRef}
          states={states}
          reportPick={reportPick}
        />
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
