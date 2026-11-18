import { type SyntheticEvent, useCallback, useRef, useState } from "react";

import { firstFramePlayed } from "@/shared/lib/diagnostics";

/** What the controls need of the `<video>`: what it's doing, and a way to act on it. */
export type Video = Pick<ReturnType<typeof useVideo>, "state" | "control">;

/**
 * The `<video>`'s ref, what it's doing, and the handlers that keep that up to date and record when
 * its first frame played.
 */
export function useVideo() {
  const ref = useRef<HTMLVideoElement>(null);
  const [state, setState] = useState({
    playing: false,
    time: 0,
    duration: 0,
    volume: 1,
    muted: false,
  });
  const sync = ({ currentTarget: video }: SyntheticEvent<HTMLVideoElement>) => {
    setState({
      playing: !video.paused,
      time: video.currentTime,
      // A stream of unknown length reports `Infinity`.
      duration: Number.isFinite(video.duration) ? video.duration : 0,
      volume: video.volume,
      muted: video.muted,
    });
  };
  const events = {
    onPlay: sync,
    onPause: sync,
    onTimeUpdate: sync,
    onDurationChange: sync,
    onVolumeChange: sync,
    onPlaying: firstFramePlayed,
  };
  /** Runs `action` on the `<video>`, from an event handler or an effect. */
  const control = useCallback((action: (video: HTMLVideoElement) => void) => {
    if (ref.current) {
      action(ref.current);
    }
  }, []);
  return { ref, state, events, control };
}
