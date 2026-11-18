import { type RefObject, useEffect } from "react";

import { useReportProgress } from "@/features/player/data/use-report-progress";
import { followVideo } from "@/features/player/lib/follow-video";

/**
 * Keeps your Watch State while the `<video>` plays Version `versionId`: it starts at
 * `startPositionMs`, and reports how far in it is every few seconds, on pause and seek, and on
 * leaving. Reports go out in the background, so playback never waits on one. A failed report is
 * sent again, and never shown.
 */
export function useWatchState(
  videoRef: RefObject<HTMLVideoElement | null>,
  versionId: bigint,
  startPositionMs: bigint,
) {
  const report = useReportProgress();

  useEffect(() => {
    const video = videoRef.current;
    return video
      ? followVideo(video, startPositionMs, (position) => {
          report({ versionId, ...position });
        })
      : undefined;
  }, [videoRef, versionId, startPositionMs, report]);
}
