import { SEEK_STEP_SECONDS, play, seekBy } from "@/features/player/lib/video-actions";

/** What the lock screen, the notification and the media keys' overlay show of what's playing. */
export interface NowPlaying {
  title: string;
  /** For an Episode, its Show and which Episode it is, shown under `title`. */
  artist?: string | undefined;
  /** The Poster shown beside it: an Episode's is its Show's. */
  posterUrl?: string | undefined;
}

/** Hands `session` what's playing in `video` and the controls of it, until the cleanup it returns. */
export function showOnLockScreen(
  session: MediaSession,
  video: HTMLVideoElement,
  nowPlaying: NowPlaying,
) {
  const { title, artist = "", posterUrl } = nowPlaying;
  const artwork = posterUrl === undefined ? [] : [{ src: posterUrl }];
  session.metadata = new MediaMetadata({ title, artist, artwork });
  const handlers = mediaActions(video);
  for (const action of MEDIA_ACTIONS) {
    session.setActionHandler(action, handlers[action]);
  }
  const showPosition = () => {
    // `setPositionState` throws on the `NaN` a video reports before it knows its length.
    if (Number.isNaN(video.duration)) {
      return;
    }
    const { duration, currentTime, playbackRate } = video;
    session.setPositionState({ duration, position: Math.min(currentTime, duration), playbackRate });
  };
  video.addEventListener("timeupdate", showPosition);
  video.addEventListener("durationchange", showPosition);
  return () => {
    video.removeEventListener("timeupdate", showPosition);
    video.removeEventListener("durationchange", showPosition);
    for (const action of MEDIA_ACTIONS) {
      session.setActionHandler(action, null);
    }
    session.metadata = null;
    session.setPositionState();
  };
}

/** Has the Next Track media key and button `playNext`, until the cleanup it returns. */
export function nextTrackPlays(session: MediaSession, playNext: () => void) {
  session.setActionHandler("nexttrack", playNext);
  return () => {
    session.setActionHandler("nexttrack", null);
  };
}

const MEDIA_ACTIONS = ["play", "pause", "stop", "seekto", "seekbackward", "seekforward"] as const;

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
    // Stopped where it is, so playing again carries on from there.
    stop: () => {
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
