import { type SyntheticEvent, useCallback, useRef, useState } from "react";

import { firstFramePlayed, stallEnded, stallStarted } from "@/shared/lib/diagnostics";

/** What the controls need of the `<video>`: what it's doing, and a way to act on it. */
export type Video = Pick<ReturnType<typeof useVideo>, "state" | "control">;

/**
 * The `<video>`'s ref, what it's doing, and the handlers that keep that up to date and record when
 * its first frame played and when it stalled.
 */
export function useVideo() {
  const ref = useRef<HTMLVideoElement>(null);
  const [state, setState] = useState({
    playing: false,
    time: 0,
    duration: 0,
    buffered: 0,
    rate: 1,
    volume: 1,
    muted: false,
    // Waiting for more of the video, from when it starts loading until it can play on.
    buffering: false,
    // Has a frame to show, from its first until a new source starts loading.
    framed: false,
  });
  const sync = ({ currentTarget: video }: SyntheticEvent<HTMLVideoElement>) => {
    setState((was) => ({
      ...was,
      playing: !video.paused,
      time: video.currentTime,
      // A stream of unknown length reports `Infinity`.
      duration: Number.isFinite(video.duration) ? video.duration : 0,
      buffered: bufferedEnd(video),
      rate: video.playbackRate,
      volume: video.volume,
      muted: video.muted,
    }));
  };
  const waiting = (buffering: boolean) => () => {
    setState((was) => ({ ...was, buffering }));
  };
  const events = {
    onPlay: sync,
    onPause: sync,
    onTimeUpdate: sync,
    // At once, so a seek bar let go of shows where it seeked rather than where it played last.
    onSeeking: sync,
    onDurationChange: sync,
    onVolumeChange: sync,
    onProgress: sync,
    onRateChange: sync,
    onLoadStart: () => {
      setState((was) => ({ ...was, buffering: true, framed: false }));
    },
    onLoadedData: () => {
      setState((was) => ({ ...was, framed: true }));
    },
    onWaiting: (event: SyntheticEvent<HTMLVideoElement>) => {
      waiting(true)();
      stallStarted(event.currentTarget);
    },
    onCanPlay: waiting(false),
    onPlaying: () => {
      waiting(false)();
      firstFramePlayed();
      stallEnded();
    },
  };
  /** Runs `action` on the `<video>`, from an event handler or an effect. */
  const control = useCallback((action: (video: HTMLVideoElement) => void) => {
    if (ref.current) {
      action(ref.current);
    }
  }, []);
  return { ref, state, events, control };
}

/** Where the loaded stretch that playing is in ends, in seconds: `currentTime` if none is loaded. */
function bufferedEnd({ buffered, currentTime }: HTMLVideoElement) {
  for (let range = 0; range < buffered.length; range += 1) {
    if (buffered.start(range) <= currentTime && currentTime <= buffered.end(range)) {
      return buffered.end(range);
    }
  }
  return currentTime;
}
