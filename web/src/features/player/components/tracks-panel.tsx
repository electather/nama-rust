import { Captions } from "lucide-react";
import type { RefObject } from "react";

import { TrackLists } from "@/features/player/components/track-lists";
import { useMaySearchSubtitles } from "@/features/player/data/use-may-search-subtitles";
import { useReportTrackPick } from "@/features/player/data/use-report-track-pick";
import type { usePlaybackPlan } from "@/features/player/hooks/use-playback-plan";
import { useShownSubtitles } from "@/features/player/hooks/use-shown-subtitles";
import { PANEL_GAP, type PanelPlace } from "@/features/player/lib/panel-place";
import type { Track } from "@/gen/nama/v1/browse_pb";
import { Button } from "@/shared/ui/button";
import { Popover, PopoverContent, PopoverTrigger } from "@/shared/ui/popover";

/**
 * Audio and subtitles side by side, as one button opens them. Picking one keeps the panel open for
 * the other. Each pick is reported, so the Server keeps it for the Show or Movie the Version is of.
 * It also shows the chosen subtitle, so it's there whether the panel is open or not. With Download
 * subtitles and a subtitle source enabled, the User can search for more.
 */
export function TracksPanel({
  tracks,
  planned,
  videoRef,
  place,
}: {
  tracks: Track[];
  planned: ReturnType<typeof usePlaybackPlan>;
  videoRef: RefObject<HTMLVideoElement | null>;
  place: PanelPlace;
}) {
  const states = useShownSubtitles(planned, videoRef);
  const reportPick = useReportTrackPick(planned.versionId);
  const maySearch = useMaySearchSubtitles();
  const { audioTracks, subtitles, burnInTracks } = planned.plan;
  // Nothing to pick: only the file's own audio, no subtitles, and none to search for.
  if (audioTracks.length < 2 && subtitles.length + burnInTracks.length === 0 && !maySearch) {
    return null;
  }
  return (
    <Popover>
      <PopoverTrigger
        render={
          <Button
            variant="overlay"
            size="icon"
            aria-label="Audio and subtitles"
            title="Audio and subtitles"
          />
        }
      >
        <Captions aria-hidden />
      </PopoverTrigger>
      <PopoverContent
        width="double"
        {...place}
        sideOffset={PANEL_GAP}
        aria-label="Audio and subtitles"
      >
        <TrackLists
          tracks={tracks}
          planned={planned}
          videoRef={videoRef}
          states={states}
          reportPick={reportPick}
          maySearch={maySearch}
        />
      </PopoverContent>
    </Popover>
  );
}
