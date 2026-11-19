import type { ReactNode, RefObject } from "react";

import { ControlRow } from "@/features/player/components/control-row";
import { SeekRow } from "@/features/player/components/seek-row";
import type { Video } from "@/features/player/hooks/use-video";

/**
 * The seek row over the control row, which holds `next` after the skips and `children` before
 * picture in picture and full screen.
 */
export function BottomBar({
  video,
  refs: { playerRef, videoRef, rowsRef },
  next,
  children,
}: {
  video: Video;
  refs: {
    playerRef: RefObject<HTMLDivElement | null>;
    videoRef: RefObject<HTMLVideoElement | null>;
    /** The seek row and the buttons, which panels open over. */
    rowsRef: RefObject<HTMLDivElement | null>;
  };
  /** Next Episode, after the skips. */
  next: ReactNode;
  children: ReactNode;
}) {
  return (
    <fieldset
      aria-label="Player controls"
      className="pointer-events-auto min-w-0 bg-linear-to-t from-scrim from-30% via-scrim/50 px-5 pt-28 pb-3.5 group-data-controls-hidden:pointer-events-none"
    >
      <div ref={rowsRef} className="flex flex-col gap-1">
        <SeekRow video={video} />
        <ControlRow video={video} refs={{ playerRef, videoRef }} next={next}>
          {children}
        </ControlRow>
      </div>
    </fieldset>
  );
}
