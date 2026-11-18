import { type LinkOptions, useNavigate } from "@tanstack/react-router";
import { type RefObject, useEffect } from "react";

import { shortcutFor } from "@/features/player/lib/shortcuts";

/** Space, the arrows, F for full screen, and Esc to go back to the title. */
export function useShortcuts(
  {
    videoRef,
    playerRef,
  }: {
    videoRef: RefObject<HTMLVideoElement | null>;
    playerRef: RefObject<HTMLDivElement | null>;
  },
  back: LinkOptions,
  wake: () => void,
) {
  const navigate = useNavigate();
  useEffect(() => {
    function onKeyDown(event: KeyboardEvent) {
      wake();
      const shortcut = shortcutFor(event);
      const video = videoRef.current;
      if (!shortcut || !video) {
        return;
      }
      event.preventDefault();
      shortcut({
        video,
        player: playerRef.current,
        back: () => {
          void navigate(back);
        },
      });
    }
    document.addEventListener("keydown", onKeyDown);
    return () => {
      document.removeEventListener("keydown", onKeyDown);
    };
  }, [videoRef, playerRef, back, wake, navigate]);
}
