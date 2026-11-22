import { PictureInPicture2 } from "lucide-react";
import { Button } from "nama-ui/components/button";
import type { RefObject } from "react";

import { usePictureInPicture } from "@/features/player/hooks/use-picture-in-picture";
import { togglePictureInPicture } from "@/features/player/lib/video-actions";

/** Plays the video in a window of its own over other apps, or back in the page, where the browser can. */
export function PictureInPictureButton({
  videoRef,
}: {
  videoRef: RefObject<HTMLVideoElement | null>;
}) {
  const inWindow = usePictureInPicture(videoRef);
  // ponytail: Firefox has its own picture-in-picture button on the video and no API for one.
  if (!document.pictureInPictureEnabled) {
    return null;
  }
  return (
    <Button
      variant="overlay"
      size="icon"
      aria-label="Picture in picture"
      title="Picture in picture"
      aria-pressed={inWindow}
      onClick={() => {
        void togglePictureInPicture(videoRef.current);
      }}
    >
      <PictureInPicture2 aria-hidden />
    </Button>
  );
}
