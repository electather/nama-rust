import type { ReactNode, RefObject } from "react";

import { BottomBar } from "@/features/player/components/bottom-bar";
import { TrackMenus } from "@/features/player/components/track-menus";
import type { usePlaybackPlan } from "@/features/player/hooks/use-playback-plan";
import type { Video } from "@/features/player/hooks/use-video";
import type { Track } from "@/gen/nama/v1/browse_pb";

/** The title bar over the video and the bottom bar under it, which fade out together. */
export function PlayerControls({
  topBar,
  video,
  playerRef,
  videoRef,
  tracks,
  planned,
}: {
  topBar: ReactNode;
  video: Video;
  playerRef: RefObject<HTMLDivElement | null>;
  videoRef: RefObject<HTMLVideoElement | null>;
  tracks: Track[];
  planned: ReturnType<typeof usePlaybackPlan>;
}) {
  return (
    // Over drawn subtitles' canvas (libbitsub's is z-10). Any key shows them, so focus rings do.
    <div className="pointer-events-none absolute inset-0 z-20 flex flex-col justify-between transition-opacity duration-500 ease-out-expo group-data-controls-hidden:opacity-0 motion-reduce:transition-none">
      {topBar}
      <BottomBar video={video} playerRef={playerRef}>
        <TrackMenus tracks={tracks} planned={planned} videoRef={videoRef} container={playerRef} />
      </BottomBar>
    </div>
  );
}
