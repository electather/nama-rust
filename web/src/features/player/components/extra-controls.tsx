import type { ReactNode, RefObject } from "react";

import { FullscreenButton } from "@/features/player/components/fullscreen-button";
import { PictureInPictureButton } from "@/features/player/components/picture-in-picture-button";

/** `children` at the bar's end, then picture in picture and full screen. */
export function ExtraControls({
  playerRef,
  videoRef,
  children,
}: {
  playerRef: RefObject<HTMLDivElement | null>;
  videoRef: RefObject<HTMLVideoElement | null>;
  children: ReactNode;
}) {
  return (
    <div className="ms-auto flex items-center gap-2">
      {children}
      {/* Phones have the system's own picture in picture. */}
      <div className="hidden sm:contents">
        <PictureInPictureButton videoRef={videoRef} />
      </div>
      <FullscreenButton playerRef={playerRef} />
    </div>
  );
}
