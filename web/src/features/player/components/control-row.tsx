import type { ReactNode, RefObject } from "react";

import { ExtraControls } from "@/features/player/components/extra-controls";
import { PlayButton } from "@/features/player/components/play-button";
import { SkipButton } from "@/features/player/components/skip-button";
import { VolumeControls } from "@/features/player/components/volume-controls";
import type { Video } from "@/features/player/hooks/use-video";

/** Play, the skips, `next` and the volume, then `children` before picture in picture and full screen. */
export function ControlRow({
  video,
  refs: { playerRef, videoRef },
  next,
  children,
}: {
  video: Video;
  refs: {
    playerRef: RefObject<HTMLDivElement | null>;
    videoRef: RefObject<HTMLVideoElement | null>;
  };
  next: ReactNode;
  children: ReactNode;
}) {
  return (
    <div className="flex items-center gap-2">
      <PlayButton video={video} />
      {/* The skips only on a touch screen: with a mouse or trackpad there's a keyboard, whose arrow
          keys skip. ponytail: a browser can't tell a keyboard is attached, so a fine pointer stands
          in for one; a tablet with a keyboard and no trackpad still shows them. */}
      <span className="contents pointer-fine:hidden">
        <SkipButton video={video} forward={false} />
        <SkipButton video={video} forward />
      </span>
      {next}
      <VolumeControls video={video} />
      <ExtraControls playerRef={playerRef} videoRef={videoRef}>
        {children}
      </ExtraControls>
    </div>
  );
}
