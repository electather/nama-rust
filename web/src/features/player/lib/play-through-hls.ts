import type Hls from "hls.js";

import { playbackFailed } from "@/shared/lib/diagnostics";

/** Plays the HLS playlist `url` in `video` through hls.js, until the cleanup it returns. */
export function playThroughHlsJs(
  video: HTMLVideoElement,
  url: string,
  onCantPlay: (video: HTMLVideoElement) => void,
) {
  let hls: Hls | undefined;
  let left = false;
  const start = async () => {
    try {
      // Fetched only for HLS, which Direct Play never needs.
      const { default: HlsJs } = await import("hls.js");
      if (left) {
        return;
      }
      hls = new HlsJs();
      hls.on(HlsJs.Events.ERROR, (_event, { fatal, details }) => {
        if (fatal) {
          playbackFailed(details);
          onCantPlay(video);
        }
      });
      hls.loadSource(url);
      hls.attachMedia(video);
    } catch {
      onCantPlay(video);
    }
  };
  void start();
  return () => {
    left = true;
    hls?.destroy();
  };
}
