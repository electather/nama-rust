import type { Transport } from "@connectrpc/connect";
import { callUnaryMethod, useTransport } from "@connectrpc/connect-query";
import { type RefObject, useEffect } from "react";

import type Hls from "hls.js";

import { PlaybackMethod, type PlaybackPlan, PlaybackService } from "@/gen/nama/v1/playback_pb";
import { playsHlsNatively } from "@/lib/device-decoding";

/**
 * How often an open plan asks for its URL. The Server stops a plan a minute after it was last asked
 * for, and a paused player, its buffer full, asks for nothing.
 */
const KEEP_ALIVE_MS = 30_000;

/**
 * Plays `url` in the `<video>`: a file when `method` Direct Plays, or else an HLS playlist, played
 * natively where the browser has HLS and through hls.js otherwise, which calls `onCantPlay` with
 * the `<video>` if it gives up. Keeps the plan `id` running until the player closes or a new plan
 * replaces it, and then tells the Server it stopped. Returns the `src` the `<video>` takes, if any.
 */
export function usePlaybackSource(
  videoRef: RefObject<HTMLVideoElement | null>,
  { id, url, method }: Pick<PlaybackPlan, "id" | "url" | "method">,
  onCantPlay: (video: HTMLVideoElement) => void,
) {
  const transport = useTransport();
  const throughHlsJs = method !== PlaybackMethod.DIRECT_PLAY && !playsHlsNatively();
  useEffect(() => {
    const video = videoRef.current;
    return throughHlsJs && video ? playThroughHlsJs(video, url, onCantPlay) : undefined;
  }, [videoRef, throughHlsJs, url, onCantPlay]);
  useEffect(() => keepPlaying(transport, id, url), [transport, id, url]);
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

/**
 * Asks for the plan `planId`'s `url` every `KEEP_ALIVE_MS`, until the cleanup it returns, which
 * tells the Server it stopped, so it ends at once rather than a minute later.
 */
function keepPlaying(transport: Transport, planId: bigint, url: string) {
  const asking = setInterval(() => {
    // `HEAD`, so a Direct Play's whole file isn't sent. A failed ask is left to the player, which
    // notices when it next needs a segment.
    fetch(url, { method: "HEAD" }).catch(() => {});
  }, KEEP_ALIVE_MS);
  return () => {
    clearInterval(asking);
    // A failed stop is left to that minute.
    callUnaryMethod(transport, PlaybackService.method.stopPlayback, { planId }).catch(() => {});
  };
}
