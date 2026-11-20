import type { ComponentProps, RefObject } from "react";

import { AudioChoices } from "@/features/player/components/audio-choices";
import { SubtitleChoices } from "@/features/player/components/subtitle-choices";
import { SubtitleSearch } from "@/features/player/components/subtitle-search";
import { TrackColumn } from "@/features/player/components/track-column";
import type { usePlaybackPlan } from "@/features/player/hooks/use-playback-plan";
import type { Track } from "@/gen/nama/v1/browse_pb";
import { PickedTrackKind } from "@/gen/nama/v1/playback_pb";

/**
 * The audio Tracks beside the subtitles, each list scrolling on its own, and under the subtitles
 * their search when the User `maySearch`.
 */
export function TrackLists({
  tracks,
  planned,
  videoRef,
  states,
  reportPick,
  maySearch,
}: {
  tracks: Track[];
  planned: ReturnType<typeof usePlaybackPlan>;
  videoRef: RefObject<HTMLVideoElement | null>;
  maySearch: boolean;
} & Pick<ComponentProps<typeof SubtitleChoices>, "states" | "reportPick">) {
  const { plan } = planned;
  return (
    <div className="grid max-h-96 min-h-0 grid-cols-2 grid-rows-1">
      <TrackColumn heading="Audio">
        <AudioChoices
          tracks={tracks}
          audio={plan.audioTracks}
          playing={plan.audioTrack}
          choose={(chosen) => {
            if (videoRef.current) {
              planned.chooseAudio(videoRef.current, chosen);
            }
            reportPick(PickedTrackKind.AUDIO, chosen);
          }}
        />
      </TrackColumn>
      <TrackColumn heading="Subtitles">
        <SubtitleChoices
          tracks={tracks}
          planned={planned}
          videoRef={videoRef}
          states={states}
          reportPick={reportPick}
        />
        {maySearch && <SubtitleSearch planned={planned} videoRef={videoRef} />}
      </TrackColumn>
    </div>
  );
}
