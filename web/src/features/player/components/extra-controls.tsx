import type { ReactNode, RefObject } from "react";

import { FullscreenButton } from "@/features/player/components/fullscreen-button";
import { VolumeControls } from "@/features/player/components/volume-controls";
import type { Video } from "@/features/player/hooks/use-video";

/** `children` beside the volume and full screen. */
export function ExtraControls({
  video,
  playerRef,
  children,
}: {
  video: Video;
  playerRef: RefObject<HTMLDivElement | null>;
  children: ReactNode;
}) {
  return (
    <div className="ms-auto flex items-center gap-1">
      {children}
      <VolumeControls video={video} />
      <FullscreenButton playerRef={playerRef} />
    </div>
  );
}
