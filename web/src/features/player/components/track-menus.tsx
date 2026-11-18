import type { RefObject } from "react";

import { AudioMenu } from "@/features/player/components/audio-menu";
import { PlaybackLimitMenu } from "@/features/player/components/playback-limit-menu";
import { SubtitlesMenu } from "@/features/player/components/subtitles-menu";
import { useReportTrackPick } from "@/features/player/data/use-report-track-pick";
import type { usePlaybackPlan } from "@/features/player/hooks/use-playback-plan";
import type { Track } from "@/gen/nama/v1/browse_pb";
import { PickedTrackKind } from "@/gen/nama/v1/playback_pb";

/**
 * The menus choosing what plays: the Version's audio Track, then its subtitles, then the Playback
 * Limit. Each audio Track picked is reported, so the Server keeps it for the Show or Movie the
 * Version is of.
 */
export function TrackMenus({
  tracks,
  planned,
  videoRef,
  container,
}: {
  tracks: Track[];
  planned: ReturnType<typeof usePlaybackPlan>;
  videoRef: RefObject<HTMLVideoElement | null>;
  container: RefObject<HTMLDivElement | null>;
}) {
  const reportPick = useReportTrackPick(planned.versionId);
  return (
    <>
      <AudioMenu
        tracks={tracks}
        offered={planned.plan.audioTracks}
        playing={planned.plan.audioTrack}
        choose={(place) => {
          if (videoRef.current) {
            planned.chooseAudio(videoRef.current, place);
          }
          reportPick(PickedTrackKind.AUDIO, place);
        }}
        container={container}
      />
      <SubtitlesMenu tracks={tracks} planned={planned} videoRef={videoRef} container={container} />
      <PlaybackLimitMenu
        offered={planned.plan.playbackLimits}
        chosen={planned.plan.playbackLimit}
        choose={(limit) => {
          if (videoRef.current) {
            planned.chooseLimit(videoRef.current, limit);
          }
        }}
        container={container}
      />
    </>
  );
}
