import { type RefObject, useEffect, useEffectEvent, useState } from "react";

import { stopDrawing } from "@/features/player/lib/device-decoding";
import { renderAss } from "@/features/player/lib/render-ass";
import { renderBitmap } from "@/features/player/lib/render-bitmap";
import { type SubtitleState, draw } from "@/features/player/lib/subtitle-render";
import { type PlaybackSubtitle, SubtitleFormat } from "@/gen/nama/v1/playback_pb";

/**
 * Draws the plan's subtitle `drawn`, one in a format this Device draws, over the `<video>`, or
 * none, and says how each drawn so far is getting on: loading until it and the fonts it's drawn
 * with are fetched and read, or failed if they can't be fetched. When the renderer won't start or
 * throws, this Device stops drawing that format and `onRendererFailed` runs. A drawn subtitle
 * stays where its Track puts it, under the controls.
 */
export function useDrawnSubtitle(
  videoRef: RefObject<HTMLVideoElement | null>,
  drawn: PlaybackSubtitle | undefined,
  onRendererFailed: () => void,
): Partial<Record<string, SubtitleState>> {
  const [states, setStates] = useState<Partial<Record<string, SubtitleState>>>({});
  const rendererFailed = useEffectEvent((format: SubtitleFormat) => {
    stopDrawing(format);
    onRendererFailed();
  });
  useEffect(() => {
    const video = videoRef.current;
    const start = (subtitle: PlaybackSubtitle, on: HTMLVideoElement) => {
      const settle = (state: SubtitleState) => {
        setStates((before) => ({ ...before, [subtitle.url]: state }));
      };
      const render = subtitle.format === SubtitleFormat.ASS ? renderAss : renderBitmap;
      return draw(
        {
          video: on,
          subtitle,
          settle,
          rendererFailed: () => {
            rendererFailed(subtitle.format);
          },
        },
        render,
      );
    };
    return drawn !== undefined && video ? start(drawn, video) : undefined;
  }, [videoRef, drawn]);
  return states;
}
