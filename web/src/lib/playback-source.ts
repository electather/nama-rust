import { type RefObject, useEffect } from "react";

import type Hls from "hls.js";

import { PlaybackMethod, type PlaybackPlan } from "@/gen/nama/v1/playback_pb";
import { playsHlsNatively } from "@/lib/device-decoding";

/**
 * Plays `url` in the `<video>`: a file when `method` Direct Plays, or else an HLS playlist, played
 * natively where the browser has HLS and through hls.js otherwise, which calls `onCantPlay` with
 * the `<video>` if it gives up. Returns the `src` the `<video>` takes, if any.
 */
export function usePlaybackSource(
  videoRef: RefObject<HTMLVideoElement | null>,
  { url, method }: Pick<PlaybackPlan, "url" | "method">,
  onCantPlay: (video: HTMLVideoElement) => void,
) {
  const throughHlsJs = method !== PlaybackMethod.DIRECT_PLAY && !playsHlsNatively();
  useEffect(() => {
    const video = videoRef.current;
    return throughHlsJs && video ? playThroughHlsJs(video, url, onCantPlay) : undefined;
  }, [videoRef, throughHlsJs, url, onCantPlay]);
  return throughHlsJs ? undefined : url;
}

/** Plays the HLS playlist `url` in `video` through hls.js, until the cleanup it returns. */
function playThroughHlsJs(
  video: HTMLVideoElement,
  url: string,
  onCantPlay: (video: HTMLVideoElement) => void,
) {
  let hls: Hls | undefined;
  let left = false;
  const start = async () => {
    // Fetched only for HLS, which Direct Play never needs.
    const { default: HlsJs } = await import("hls.js");
    if (left) return;
    hls = new HlsJs();
    hls.on(HlsJs.Events.ERROR, (_event, { fatal }) => {
      if (fatal) onCantPlay(video);
    });
    hls.loadSource(url);
    hls.attachMedia(video);
  };
  start().catch(() => {
    onCantPlay(video);
  });
  return () => {
    left = true;
    hls?.destroy();
  };
}
