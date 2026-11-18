import { useReducer } from "react";

import type { NextEpisode } from "@/features/player/lib/next-episode";

/**
 * What autoplay keeps of an Episode across its Playback Plans, which each play in a new `<video>`:
 * `next`, and whether the player had input or the countdown was cancelled.
 */
export function useAutoplay(next: NextEpisode | undefined) {
  const [touched, touch] = useReducer(() => true, false);
  const [cancelled, cancel] = useReducer(() => true, false);
  return { next, touched, touch, cancelled, cancel };
}

/** What `useAutoplay` keeps of an Episode. */
export type Autoplay = ReturnType<typeof useAutoplay>;
