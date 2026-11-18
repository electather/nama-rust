import type { PointerEvent } from "react";

import { togglePlay } from "@/features/player/lib/video-actions";

/** A click pauses or plays; a tap only brings the controls back. */
export function clickToToggle(event: PointerEvent<HTMLVideoElement>) {
  if (event.pointerType === "mouse") {
    togglePlay(event.currentTarget);
  }
}
