import { type RefObject, useEffect } from "react";

import { shiftCues } from "@/features/player/lib/shift-cues";

/**
 * Shows the cues of the `<video>`'s `<track>` at the URL `shown` `offsetMs` later than its file
 * times them, once they load and whenever the offset changes.
 */
export function useCueOffset(
  videoRef: RefObject<HTMLVideoElement | null>,
  shown: string | undefined,
  offsetMs: number,
) {
  useEffect(() => {
    const video = videoRef.current;
    const shift = () => {
      for (const element of video?.querySelectorAll("track") ?? []) {
        if (element.getAttribute("src") === shown) {
          shiftCues(element.track, offsetMs);
        }
      }
    };
    shift();
    // A `<track>`'s load doesn't bubble, so the `<video>` catches it on the way down.
    video?.addEventListener("load", shift, true);
    return () => {
      video?.removeEventListener("load", shift, true);
    };
  }, [videoRef, shown, offsetMs]);
}
