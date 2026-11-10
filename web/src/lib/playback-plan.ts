import { create } from "@bufbuild/protobuf";
import { callUnaryMethod, useTransport } from "@connectrpc/connect-query";
import { useSuspenseQuery } from "@tanstack/react-query";
import { type Dispatch, type SetStateAction, useCallback, useState } from "react";

import {
  PlaybackLimit,
  type PlaybackMethod,
  PlaybackPlanSchema,
  PlaybackService,
} from "@/gen/nama/v1/playback_pb";
import { findDeviceDecoding } from "@/lib/device-decoding";
import type { WatchState } from "@/gen/nama/v1/watch_state_pb";
import { toMs } from "@/lib/watch-state";

/** What the player asks a plan for, beside the Version. */
interface Asking {
  failedMethods: PlaybackMethod[];
  startPositionMs: bigint;
  /** The chosen audio Track's place in the Version's Tracks, `undefined` being the file's own. */
  audioTrack: number | undefined;
  /** The Playback Limit chosen, `PLAYBACK_LIMIT_UNSPECIFIED` until the User picks one. */
  playbackLimit: PlaybackLimit;
}

/**
 * The Server's Playback Plan for Version `versionId` on this Device, the position in milliseconds
 * it starts from, at first where `watchState` stopped, and `onCantPlay` for when the browser fails
 * to play it. `onCantPlay` asks for a new plan from where the `<video>` stands, without the method
 * that failed, or else sets `gaveUp`: only once per method, so a Server that plans it again can't
 * make the player ask forever. `chooseAudio` asks for a new plan from there with the audio Track
 * at a place in the Version's Tracks, which `audioTrack` then holds, and `chooseLimit` with a
 * Playback Limit, which `playbackLimit` then holds.
 */
export function usePlaybackPlan({
  versionId,
  watchState,
}: {
  versionId: bigint;
  watchState: WatchState | undefined;
}) {
  // Where it stood on opening, until a method fails or the User chooses something else: a new plan
  // would reload the `<video>`, so the refetches each Watch State report causes mustn't ask again.
  const [asking, setAsking] = useState<Asking>(() => ({
    failedMethods: [],
    startPositionMs: watchState && !watchState.played ? watchState.positionMs : 0n,
    audioTrack: undefined,
    playbackLimit: PlaybackLimit.PLAYBACK_LIMIT_UNSPECIFIED,
  }));
  const { failedMethods, startPositionMs, audioTrack, playbackLimit } = asking;
  const [gaveUp, setGaveUp] = useState(false);
  const plan = usePlan(versionId, asking);
  // Stable until the plan changes, so hls.js isn't restarted each time the player renders.
  const onCantPlay = useCallback(
    (video: HTMLVideoElement) => {
      // So a failure that leaves its buffer playing isn't heard behind the loading screen.
      video.pause();
      // Read here: React may run the updater later, when the `<video>` has moved on.
      const resumeFrom = resumeFromMs(video, startPositionMs);
      if (failedMethods.includes(plan.method)) setGaveUp(true);
      // hls.js and the `<video>` may both report one failure: only the first asks again.
      else
        setAsking((before) =>
          before.failedMethods.includes(plan.method)
            ? before
            : {
                ...before,
                failedMethods: [...before.failedMethods, plan.method],
                startPositionMs: resumeFrom,
              },
        );
    },
    [failedMethods, plan.method, startPositionMs],
  );
  const chosen = { audioTrack, playbackLimit, ...choosers(setAsking, startPositionMs) };
  return { plan, startPositionMs, gaveUp, onCantPlay, ...chosen };
}

/**
 * `chooseAudio` and `chooseLimit`, which `setAsking` for a new plan with the User's choice, from
 * where the `<video>` stands, or else from `startPositionMs`.
 */
function choosers(setAsking: Dispatch<SetStateAction<Asking>>, startPositionMs: bigint) {
  const askFromHere = (
    video: HTMLVideoElement,
    choice: Partial<Pick<Asking, "audioTrack" | "playbackLimit">>,
  ) => {
    // As for a failure, so it isn't heard behind the loading screen.
    video.pause();
    const resumeFrom = resumeFromMs(video, startPositionMs);
    setAsking((before) => ({ ...before, ...choice, startPositionMs: resumeFrom }));
  };
  return {
    chooseAudio: (video: HTMLVideoElement, place: number) => {
      askFromHere(video, { audioTrack: place });
    },
    chooseLimit: (video: HTMLVideoElement, limit: PlaybackLimit) => {
      askFromHere(video, { playbackLimit: limit });
    },
  };
}

/** The plan for Version `versionId` as `asking` says, asked for once and never reused after. */
function usePlan(versionId: bigint, asking: Asking) {
  const transport = useTransport();
  const { failedMethods, startPositionMs, audioTrack, playbackLimit } = asking;
  return useSuspenseQuery({
    queryKey: [
      "planPlayback",
      String(versionId),
      failedMethods,
      String(startPositionMs),
      audioTrack,
      playbackLimit,
    ],
    queryFn: async () => {
      const deviceDecoding = await findDeviceDecoding();
      const input = { versionId, deviceDecoding, ...asking };
      const response = await callUnaryMethod(transport, PlaybackService.method.planPlayback, input);
      return create(PlaybackPlanSchema, response.plan);
    },
    staleTime: "static",
    gcTime: 0,
  }).data;
}

/**
 * Where `video` stands in milliseconds, or `startPositionMs` if it never loaded far enough to have
 * started there.
 */
function resumeFromMs(video: HTMLVideoElement, startPositionMs: bigint) {
  if (video.readyState < HTMLMediaElement.HAVE_METADATA) return startPositionMs;
  return toMs(video.currentTime);
}
