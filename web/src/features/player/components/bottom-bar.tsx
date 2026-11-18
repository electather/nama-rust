import type { ReactNode, RefObject } from "react";

import { ExtraControls } from "@/features/player/components/extra-controls";
import { PlayButton } from "@/features/player/components/play-button";
import type { Video } from "@/features/player/hooks/use-video";
import { clock } from "@/features/player/lib/video-actions";
import { Slider } from "@/shared/ui/slider";

/** The position, Play and the time, then `children` beside the volume and full screen. */
export function BottomBar({
  video,
  playerRef,
  children,
}: {
  video: Video;
  playerRef: RefObject<HTMLDivElement | null>;
  children: ReactNode;
}) {
  const { time, duration } = video.state;
  return (
    <div className="pointer-events-auto bg-linear-to-t from-scrim px-2 pt-10 pb-2 group-data-controls-hidden:pointer-events-none sm:px-4 sm:pb-4">
      <div className="px-2">
        <Slider
          label="Position"
          valueText={`${clock(time)} of ${clock(duration)}`}
          disabled={duration === 0}
          value={Math.min(time, duration)}
          max={duration === 0 ? 1 : duration}
          onValueChange={(seconds) => {
            video.control((element) => {
              element.currentTime = seconds;
            });
          }}
        />
      </div>
      <div className="flex items-center gap-1">
        <PlayButton video={video} />
        <span className="ms-2 text-sm tabular-nums">
          {clock(time)} / {clock(duration)}
        </span>
        <ExtraControls video={video} playerRef={playerRef}>
          {children}
        </ExtraControls>
      </div>
    </div>
  );
}
