import { toMs } from "@/features/player/lib/to-ms";
import type { ReportProgressRequest } from "@/gen/nama/v1/watch_state_pb";

/** How often to report while playing. */
const REPORT_EVERY_MS = 10_000;

/**
 * Starts `video` from `startPositionMs` and passes how far in it is to `report` every few seconds,
 * on pause and seek, when the page is hidden or closed, and in the cleanup it returns.
 */
export function followVideo(
  video: HTMLVideoElement,
  startPositionMs: bigint,
  report: (position: Pick<ReportProgressRequest, "positionMs" | "durationMs">) => void,
) {
  let reportedAt = 0;
  const send = () => {
    // Until the file's length is known there's nothing to report.
    if (!(video.duration > 0) || !Number.isFinite(video.duration)) {
      return;
    }
    reportedAt = Date.now();
    report({ positionMs: toMs(video.currentTime), durationMs: toMs(video.duration) });
  };
  let resumed = false;
  // Once: a reloaded source mustn't seek back to where it started.
  const resume = () => {
    if (resumed) {
      return;
    }
    resumed = true;
    if (startPositionMs > 0n) {
      video.currentTime = Number(startPositionMs) / 1000;
    }
  };
  const sendEveryFewSeconds = () => {
    if (Date.now() - reportedAt >= REPORT_EVERY_MS) {
      send();
    }
  };

  const listeners = [
    ["loadedmetadata", resume],
    ["timeupdate", sendEveryFewSeconds],
    ["pause", send],
    ["seeked", send],
  ] as const;
  // Closing the tab unmounts nothing, and a phone may discard a page it hid without ever firing
  // `pagehide`, so going out of sight is the last sure moment. ponytail: best effort, as the page
  // may go before it's sent.
  const sendIfHidden = () => {
    if (document.visibilityState === "hidden") {
      send();
    }
  };
  for (const [type, listener] of listeners) {
    video.addEventListener(type, listener);
  }
  document.addEventListener("visibilitychange", sendIfHidden);
  window.addEventListener("pagehide", send);
  if (video.readyState >= HTMLMediaElement.HAVE_METADATA) {
    resume();
  }
  return () => {
    for (const [type, listener] of listeners) {
      video.removeEventListener(type, listener);
    }
    document.removeEventListener("visibilitychange", sendIfHidden);
    window.removeEventListener("pagehide", send);
    send();
  };
}
