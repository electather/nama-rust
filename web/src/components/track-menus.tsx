import type { RefObject } from "react";

import { AudioMenu } from "@/components/audio-menu";
import type { Track } from "@/gen/nama/v1/browse_pb";
import { PickedTrackKind } from "@/gen/nama/v1/playback_pb";
import { PlaybackLimitMenu } from "@/components/playback-limit-menu";
import { SubtitlesMenu } from "@/components/subtitles-menu";
import type { usePlaybackPlan } from "@/lib/playback-plan";
import { usePickReporter } from "@/lib/subtitles";

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
  const reportPick = usePickReporter(planned.versionId);
  return (
    <>
      <AudioMenu
        tracks={tracks}
        offered={planned.plan.audioTracks}
        playing={planned.plan.audioTrack}
        choose={(place) => {
          if (videoRef.current) planned.chooseAudio(videoRef.current, place);
          reportPick(PickedTrackKind.AUDIO, place);
        }}
        container={container}
      />
      <SubtitlesMenu tracks={tracks} planned={planned} videoRef={videoRef} container={container} />
      <PlaybackLimitMenu
        offered={planned.plan.playbackLimits}
        chosen={planned.plan.playbackLimit}
        choose={(limit) => {
          if (videoRef.current) planned.chooseLimit(videoRef.current, limit);
        }}
        container={container}
      />
    </>
  );
}
