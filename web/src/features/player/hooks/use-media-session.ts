import { type LinkOptions, useNavigate } from "@tanstack/react-router";
import { type RefObject, useEffect } from "react";

import {
  type NowPlaying,
  nextTrackPlays,
  showOnLockScreen,
} from "@/features/player/lib/lock-screen";

/**
 * Shows `nowPlaying` and the position on the lock screen and in the notification, whose buttons
 * and the media keys play, pause, stop and seek the video, and Next Track plays `next`, until the
 * player closes. Does nothing in a browser without the Media Session API.
 */
export function useMediaSession(
  videoRef: RefObject<HTMLVideoElement | null>,
  { title, artist, posterUrl }: NowPlaying,
  next: LinkOptions | undefined,
) {
  const navigate = useNavigate();
  useEffect(
    () =>
      next && "mediaSession" in navigator
        ? nextTrackPlays(navigator.mediaSession, () => {
            void navigate(next);
          })
        : undefined,
    [next, navigate],
  );
  useEffect(() => {
    const video = videoRef.current;
    return video && "mediaSession" in navigator
      ? showOnLockScreen(navigator.mediaSession, video, { title, artist, posterUrl })
      : undefined;
  }, [videoRef, title, artist, posterUrl]);
}
