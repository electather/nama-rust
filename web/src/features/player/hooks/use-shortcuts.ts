import { type LinkOptions, useNavigate } from "@tanstack/react-router";
import { type RefObject, useEffect } from "react";

import { shortcutFor } from "@/features/player/lib/shortcuts";

/** What the player's keys need besides the video: where they go and what they show. */
export interface ShortcutParts {
  /** The title page Esc goes back to. */
  back: LinkOptions;
  /** The next Episode Shift+N plays, when there is one. */
  next: LinkOptions | undefined;
  /** Called on every key, as input that shows the controls. */
  wake: () => void;
  /** Turns subtitles on or off in `video`. */
  captions: (video: HTMLVideoElement) => void;
  /** How long one frame shows, in seconds. */
  frame: number;
}

/** YouTube's keys for the player (`shortcuts.ts`), and Esc to go back to the title. */
export function useShortcuts(
  {
    videoRef,
    playerRef,
  }: {
    videoRef: RefObject<HTMLVideoElement | null>;
    playerRef: RefObject<HTMLDivElement | null>;
  },
  { back, next, wake, captions, frame }: ShortcutParts,
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
        next:
          next &&
          (() => {
            void navigate(next);
          }),
        captions: () => {
          captions(video);
        },
        frame,
      });
    }
    document.addEventListener("keydown", onKeyDown);
    return () => {
      document.removeEventListener("keydown", onKeyDown);
    };
  }, [videoRef, playerRef, back, next, wake, captions, frame, navigate]);
}
