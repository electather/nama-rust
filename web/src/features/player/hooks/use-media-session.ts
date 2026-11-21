import { type LinkOptions, useNavigate } from "@tanstack/react-router";
import { type RefObject, useEffect } from "react";

import {
  type NowPlaying,
  type TrackKey,
  showOnLockScreen,
  trackKeyPlays,
} from "@/features/player/lib/lock-screen";

/**
 * Shows `nowPlaying` and the position on the lock screen and in the notification, whose buttons
 * and the media keys play, pause, stop and seek the video, and Next and Previous Track play the
 * `next` and `previous` Episodes, until the player closes. Does nothing in a browser without the
 * Media Session API.
 */
export function useMediaSession(
  videoRef: RefObject<HTMLVideoElement | null>,
  { title, artist, posterUrl }: NowPlaying,
  { next, previous }: { next: LinkOptions | undefined; previous: LinkOptions | undefined },
) {
  useTrackKey("nexttrack", next);
  useTrackKey("previoustrack", previous);
  useEffect(() => {
    const video = videoRef.current;
    return video && "mediaSession" in navigator
      ? showOnLockScreen(navigator.mediaSession, video, { title, artist, posterUrl })
      : undefined;
  }, [videoRef, title, artist, posterUrl]);
}

/** Has the `track` media key open `episode`, while there is one. */
function useTrackKey(track: TrackKey, episode: LinkOptions | undefined) {
  const navigate = useNavigate();
  useEffect(
    () =>
      episode && "mediaSession" in navigator
        ? trackKeyPlays(navigator.mediaSession, track, () => {
            void navigate(episode);
          })
        : undefined,
    [track, episode, navigate],
  );
}
