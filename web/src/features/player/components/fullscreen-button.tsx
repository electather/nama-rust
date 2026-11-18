import { Maximize, Minimize } from "lucide-react";
import type { RefObject } from "react";

import { useIsFullscreen } from "@/features/player/hooks/use-is-fullscreen";
import { toggleFullscreen } from "@/features/player/lib/video-actions";
import { Button } from "@/shared/ui/button";

/** Makes the player full screen, or not, where the browser can. */
export function FullscreenButton({ playerRef }: { playerRef: RefObject<HTMLDivElement | null> }) {
  const isFullscreen = useIsFullscreen();
  // ponytail: iPhones can't make a page full screen, only the video with its own controls.
  if (!document.fullscreenEnabled) {
    return null;
  }
  return (
    <Button
      variant="overlay"
      size="icon"
      aria-label={isFullscreen ? "Exit full screen" : "Full screen"}
      onClick={() => {
        toggleFullscreen(playerRef.current);
      }}
    >
      {isFullscreen ? <Minimize aria-hidden /> : <Maximize aria-hidden />}
    </Button>
  );
}
