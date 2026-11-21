import { type RefObject, useEffect } from "react";

import { useForgetPlan } from "@/features/player/data/use-forget-plan";
import { useStopPlan } from "@/features/player/data/use-stop-plan";
import { playsHlsNatively } from "@/features/player/lib/device-decoding";
import { keepPlaying } from "@/features/player/lib/plan-lifetime";
import { playThroughHlsJs } from "@/features/player/lib/play-through-hls";
import { PlaybackMethod, type PlaybackPlan } from "@/gen/nama/v1/playback_pb";

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
  const tellStopped = useStopPlan();
  const forget = useForgetPlan();
  const throughHlsJs = method !== PlaybackMethod.DIRECT_PLAY && !playsHlsNatively();
  useEffect(() => {
    const video = videoRef.current;
    return throughHlsJs && video ? playThroughHlsJs(video, url, onCantPlay) : undefined;
  }, [videoRef, throughHlsJs, url, onCantPlay]);
  useEffect(() => keepPlaying(tellStopped, forget, { id, url }), [tellStopped, forget, id, url]);
  return throughHlsJs ? undefined : url;
}
