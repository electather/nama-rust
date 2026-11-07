import { useMutation } from "@connectrpc/connect-query";
import { type QueryClient, useQueryClient } from "@tanstack/react-query";
import { type RefObject, useEffect, useState } from "react";

import {
  type ReportProgressRequest,
  type WatchState,
  WatchStateService,
} from "@/gen/nama/v1/watch_state_pb";

import { browseKey } from "./browse";
import { retryIfUnreachable } from "./connect-errors";

/**
 * Marks a Movie, Episode, Season or Show played or unplayed, then refetches what's on screen so
 * every poster, Episode and Continue Watching shows it.
 */
export function useSetPlayed() {
  const queryClient = useQueryClient();
  const { mutate } = useMutation(WatchStateService.method.setPlayed, {
    onSuccess: () => queryClient.invalidateQueries({ queryKey: browseKey }),
  });
  return mutate;
}

/** How often to report while playing. */
const REPORT_EVERY_MS = 10_000;

/**
 * Keeps your Watch State while the `<video>` plays Version `versionId`: it starts where
 * `watchState` stopped, unless that was played, and reports how far in it is every few seconds,
 * on pause and seek, and on leaving. Reports go out in the background, so playback never waits
 * on one. A failed report is sent again, and never shown.
 */
export function useWatchState(
  videoRef: RefObject<HTMLVideoElement | null>,
  versionId: bigint,
  watchState: WatchState | undefined,
) {
  // Where it stood on opening: the refetches each report causes mustn't seek it back.
  const [resumeFrom] = useState(watchState);
  const queryClient = useQueryClient();
  const { mutate: report } = useMutation(WatchStateService.method.reportProgress, {
    retry: retryIfUnreachable,
    // One at a time, in order, so a retried report never lands after a newer one.
    scope: { id: "report-progress" },
    onSuccess: () => refreshWatchState(queryClient),
  });

  useEffect(() => {
    const video = videoRef.current;
    return video
      ? followVideo(video, resumeFrom, (position) => {
          report({ versionId, ...position });
        })
      : undefined;
  }, [videoRef, versionId, resumeFrom, report]);
}

type Position = Pick<ReportProgressRequest, "positionMs" | "durationMs">;

/**
 * Starts `video` from `resumeFrom` and passes how far in it is to `report` every few seconds, on
 * pause and seek, when the page is hidden or closed, and in the cleanup it returns.
 */
function followVideo(
  video: HTMLVideoElement,
  resumeFrom: WatchState | undefined,
  report: (position: Position) => void,
) {
  let reportedAt = 0;
  const send = () => {
    // Until the file's length is known there's nothing to report.
    if (!(video.duration > 0) || !Number.isFinite(video.duration)) return;
    reportedAt = Date.now();
    report({ positionMs: toMs(video.currentTime), durationMs: toMs(video.duration) });
  };
  let resumed = false;
  // Once: a reloaded source mustn't seek back to where it stood on opening.
  const resume = () => {
    if (resumed) return;
    resumed = true;
    if (resumeFrom && !resumeFrom.played) video.currentTime = Number(resumeFrom.positionMs) / 1000;
  };
  const sendEveryFewSeconds = () => {
    if (Date.now() - reportedAt >= REPORT_EVERY_MS) send();
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
    if (document.visibilityState === "hidden") send();
  };
  for (const [type, listener] of listeners) video.addEventListener(type, listener);
  document.addEventListener("visibilitychange", sendIfHidden);
  window.addEventListener("pagehide", send);
  if (video.readyState >= HTMLMediaElement.HAVE_METADATA) resume();
  return () => {
    for (const [type, listener] of listeners) video.removeEventListener(type, listener);
    document.removeEventListener("visibilitychange", sendIfHidden);
    window.removeEventListener("pagehide", send);
    send();
  };
}

function toMs(seconds: number) {
  return BigInt(Math.round(seconds * 1000));
}

/**
 * Refreshes what browsing shows after a report. What's off screen is dropped, so opening it again,
 * such as reopening the player, fetches where you are now instead of starting from a stale copy
 * (the loaders never refetch what's cached). ponytail: that drops every page you browsed, so Back
 * fetches it again, and the player's own title refetches after each report; keep only the played
 * title's queries fresh if that ever shows.
 */
function refreshWatchState(queryClient: QueryClient) {
  queryClient.removeQueries({ queryKey: browseKey, type: "inactive" });
  return queryClient.invalidateQueries({ queryKey: browseKey });
}
