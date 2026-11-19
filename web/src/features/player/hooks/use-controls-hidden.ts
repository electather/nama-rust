import { type RefObject, useEffect } from "react";

import { useAwake } from "@/features/player/hooks/use-awake";
import type { Video } from "@/features/player/hooks/use-video";
import { clearOfControls } from "@/features/player/lib/clear-of-controls";

// ponytail: percentages of the video's height, so about 48px from the bottom once the controls
// hide; measure the bar if a short window crowds them.
const ABOVE_CONTROLS = 85;
const CONTROLS_HIDDEN = 95;

/**
 * Whether the controls hide, as they do while it plays and the pointer and keyboard rest, or else
 * `undefined`, and `wake` to show them. Keeps the `<video>`'s subtitle cues clear of them, and
 * lowers the cues while they hide.
 */
export function useControlsHidden(
  {
    playerRef,
    videoRef: ref,
  }: {
    playerRef: RefObject<HTMLElement | null>;
    videoRef: RefObject<HTMLVideoElement | null>;
  },
  { state }: Video,
) {
  const [awake, wake] = useAwake(playerRef);
  const hidden = state.playing && !awake;
  useEffect(() => {
    const video = ref.current;
    const line = hidden ? CONTROLS_HIDDEN : ABOVE_CONTROLS;
    const place = () => {
      for (const track of video?.textTracks ?? []) {
        clearOfControls(track, line);
      }
    };
    place();
    // A `<track>`'s load doesn't bubble, so the `<video>` catches it on the way down.
    video?.addEventListener("load", place, true);
    return () => {
      video?.removeEventListener("load", place, true);
    };
  }, [ref, hidden]);
  return { hidden: hidden || undefined, wake };
}
