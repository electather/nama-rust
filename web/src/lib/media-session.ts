import { type RefObject, useEffect } from "react";

import { SEEK_STEP_SECONDS, play, seekBy } from "./playback";

/** What the lock screen, the notification and the media keys' overlay show of what's playing. */
export interface NowPlaying {
  title: string;
  /** For an Episode, its Show and which Episode it is. */
  artist?: string | undefined;
  artworkUrl?: string | undefined;
}

/**
 * Shows `nowPlaying` and the position on the lock screen and in the notification, whose buttons
 * and the media keys play, pause and seek the video, until the player closes. Does nothing in a
 * browser without the Media Session API.
 */
export function useMediaSession(
  videoRef: RefObject<HTMLVideoElement | null>,
  { title, artist, artworkUrl }: NowPlaying,
) {
  useEffect(() => {
    const video = videoRef.current;
    return video && "mediaSession" in navigator
      ? showOnLockScreen(navigator.mediaSession, video, { title, artist, artworkUrl })
      : undefined;
  }, [videoRef, title, artist, artworkUrl]);
}

/** Hands `session` what's playing in `video` and the controls of it, until the cleanup it returns. */
function showOnLockScreen(session: MediaSession, video: HTMLVideoElement, nowPlaying: NowPlaying) {
  const { title, artist = "", artworkUrl } = nowPlaying;
  const artwork = artworkUrl === undefined ? [] : [{ src: artworkUrl }];
  session.metadata = new MediaMetadata({ title, artist, artwork });
  const handlers = mediaActions(video);
  for (const action of MEDIA_ACTIONS) session.setActionHandler(action, handlers[action]);
  const showPosition = () => {
    // `setPositionState` throws on the `NaN` a video reports before it knows its length.
    if (Number.isNaN(video.duration)) return;
    const { duration, currentTime, playbackRate } = video;
    session.setPositionState({ duration, position: Math.min(currentTime, duration), playbackRate });
  };
  video.addEventListener("timeupdate", showPosition);
  video.addEventListener("durationchange", showPosition);
  return () => {
    video.removeEventListener("timeupdate", showPosition);
    video.removeEventListener("durationchange", showPosition);
    for (const action of MEDIA_ACTIONS) session.setActionHandler(action, null);
    session.metadata = null;
    session.setPositionState();
  };
}

const MEDIA_ACTIONS = ["play", "pause", "seekto", "seekbackward", "seekforward"] as const;

function mediaActions(
  video: HTMLVideoElement,
): Record<(typeof MEDIA_ACTIONS)[number], MediaSessionActionHandler> {
  return {
    play: () => {
      play(video);
    },
    pause: () => {
      video.pause();
    },
    seekto: ({ seekTime = video.currentTime }) => {
      video.currentTime = seekTime;
    },
    seekbackward: ({ seekOffset = SEEK_STEP_SECONDS }) => {
      seekBy(video, -seekOffset);
    },
    seekforward: ({ seekOffset = SEEK_STEP_SECONDS }) => {
      seekBy(video, seekOffset);
    },
  };
}
