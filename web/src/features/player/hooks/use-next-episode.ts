import { useNavigate } from "@tanstack/react-router";
import { useCallback, useEffect } from "react";

import type { Autoplay } from "@/features/player/hooks/use-autoplay";
import type { Video } from "@/features/player/hooks/use-video";
import {
  dueNext,
  endsEarly,
  isStillWatchingDue,
  nextAt,
  streakAfter,
} from "@/features/player/lib/next-episode";
import { play } from "@/features/player/lib/video-actions";

/**
 * The countdown to the next Episode, and "Are you still watching?" before it, for one `<video>`
 * whose Credits start at `credits` seconds, if known. The player calls `onInput` on pointer or key
 * input, which also calls `wake`, and `playNext` when the video ends.
 */
export function useNextEpisode(
  autoplay: Autoplay,
  video: Video,
  { wake, credits }: { wake: () => void; credits: number | undefined },
) {
  const navigate = useNavigate();
  const { next, touched, touch, cancelled } = autoplay;
  const { control, state } = video;
  const due = cancelled ? undefined : dueNext(next, state, credits);
  const asking = isStillWatchingDue(due, touched);
  const at = nextAt(state.duration, credits);
  const countdown = asking ? undefined : due;
  const early = endsEarly(countdown, state, at);
  useEffect(() => {
    if (early) {
      void navigate(early.link(streakAfter(early, touched)));
    }
  }, [early, navigate, touched]);
  // Again whenever it starts playing: until it's answered, nothing plays, whatever pressed Play.
  useEffect(() => {
    if (asking && state.playing) {
      control((element) => {
        element.pause();
      });
    }
  }, [asking, state.playing, control]);
  // Input while it asks isn't an answer: only Keep watching is.
  const onInput = useCallback(() => {
    wake();
    if (!asking) {
      touch();
    }
  }, [wake, asking, touch]);

  return {
    onInput,
    playNext: () => {
      // The question still waits for an answer if the video ends under it.
      if (next && !cancelled && !asking) {
        void navigate(next.link(streakAfter(next, touched)));
      }
    },
    cancel: autoplay.cancel,
    countdown,
    secondsLeft: Math.max(0, at - state.time),
    inCredits: credits !== undefined,
    asking,
    keepWatching: () => {
      touch();
      control(play);
    },
  };
}
