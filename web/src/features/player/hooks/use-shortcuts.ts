import { type LinkOptions, useNavigate } from "@tanstack/react-router";
import { type RefObject, useEffect } from "react";

import { useGoBack } from "@/features/player/hooks/use-go-back";
import type { SeekSection } from "@/features/player/lib/seek-sections";
import { shortcutFor } from "@/features/player/lib/shortcuts";

/** What the player's keys need besides the video: where they go and what they show. */
export interface ShortcutParts {
  /** The title page Esc goes back to, when the player was opened by its link. */
  back: LinkOptions;
  /** The next Episode Shift+N plays, when there is one. */
  next: LinkOptions | undefined;
  /** The previous Episode Shift+P plays, when there is one. */
  previous: LinkOptions | undefined;
  /** Called on every key, as input that shows the controls. */
  wake: () => void;
  captions: (video: HTMLVideoElement) => void;
  /** How long one frame shows, in seconds. */
  frame: number;
  /** The seek bar's sections, which Ctrl/⌥ with Left and Right go to. */
  sections: SeekSection[];
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
  { back, next, previous, wake, captions, frame, sections }: ShortcutParts,
) {
  const navigate = useNavigate();
  const goBack = useGoBack(back);
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
        back: goBack,
        next: opener(navigate, next),
        previous: opener(navigate, previous),
        captions: () => {
          captions(video);
        },
        frame,
        sections,
      });
    }
    document.addEventListener("keydown", onKeyDown);
    return () => {
      document.removeEventListener("keydown", onKeyDown);
    };
  }, [videoRef, playerRef, goBack, next, previous, wake, captions, frame, sections, navigate]);
}

/** What opens `link`, when there is one. */
function opener(navigate: ReturnType<typeof useNavigate>, link: LinkOptions | undefined) {
  return (
    link &&
    (() => {
      void navigate(link);
    })
  );
}
