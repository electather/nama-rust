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
import { RefusalReason } from "@/gen/nama/v1/refusal_pb";
import { findDeviceDecoding } from "@/lib/device-decoding";
import type { WatchState } from "@/gen/nama/v1/watch_state_pb";
import { refusalOf } from "@/lib/forms";
import { toMs } from "@/lib/watch-state";

/** What the player asks a plan for, beside the Version. */
interface Asking {
  failedMethods: PlaybackMethod[];
  startPositionMs: bigint;
  /** The chosen audio Track's place in the Version's Tracks, `undefined` being the file's own. */
  audioTrack: number | undefined;
  /**
   * The Playback Limit chosen, `PLAYBACK_LIMIT_UNSPECIFIED` until the User picks one, when the
   * Server may pick its default for remote playback.
   */
  playbackLimit: PlaybackLimit;
  /** The place of the subtitle Track burned into the picture, if any, in the Version's Tracks. */
  subtitleTrack: number | undefined;
}

/**
 * A subtitle the User chose: a text one by the URL the player shows it from, one to burn in by its
 * place in the Version's Tracks, or `undefined` for none.
 */
export type SubtitleChoice = string | number | undefined;

/**
 * The Server's Playback Plan for Version `versionId` on this Device, the position in milliseconds
 * it starts from, at first where `watchState` stopped, and `onCantPlay` for when the browser fails
 * to play it. `onCantPlay` asks for a new plan from where the `<video>` stands, without the method
 * that failed, or else sets `gaveUp`: only once per method, so a Server that plans it again can't
 * make the player ask forever. `chooseAudio` asks for a new plan from there with the audio Track
 * at a place in the Version's Tracks, which `audioTrack` then holds, and `chooseLimit` with a
 * Playback Limit, which the plan's `playbackLimit` then holds. `chooseSubtitle` shows a text subtitle, which
 * `shownSubtitle` then holds, or asks for a new plan from there with a subtitle Track burned in,
 * which `subtitleTrack` then holds.
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
    subtitleTrack: undefined,
  }));
  const { failedMethods, startPositionMs, audioTrack, subtitleTrack } = asking;
  const askFromHere = asker(setAsking, startPositionMs);
  const subtitles = useSubtitleChoice(askFromHere, subtitleTrack);
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
  const chosen = { audioTrack, subtitleTrack, ...choosers(askFromHere) };
  return { plan, startPositionMs, gaveUp, onCantPlay, ...chosen, ...subtitles };
}

/** What the User chose, as `Asking` holds it. */
type Choice = Partial<Pick<Asking, "audioTrack" | "playbackLimit" | "subtitleTrack">>;

/**
 * What asks for a new plan with the User's choice, from where the `<video>` stands, or else from
 * `startPositionMs`, by `setAsking`.
 */
function asker(setAsking: Dispatch<SetStateAction<Asking>>, startPositionMs: bigint) {
  return (video: HTMLVideoElement, choice: Choice) => {
    // As for a failure, so it isn't heard behind the loading screen.
    video.pause();
    const resumeFrom = resumeFromMs(video, startPositionMs);
    setAsking((before) => ({ ...before, ...choice, startPositionMs: resumeFrom }));
  };
}

/** `chooseAudio` and `chooseLimit`, which `askFromHere` with the User's choice. */
function choosers(askFromHere: ReturnType<typeof asker>) {
  return {
    chooseAudio: (video: HTMLVideoElement, place: number) => {
      askFromHere(video, { audioTrack: place });
    },
    chooseLimit: (video: HTMLVideoElement, limit: PlaybackLimit) => {
      askFromHere(video, { playbackLimit: limit });
    },
  };
}

/**
 * The text subtitle shown, by URL, kept here so the `<video>` a new plan plays in shows it too,
 * and `chooseSubtitle`, which shows one, and will `askFromHere` when what's burned in changes from
 * `subtitleTrack`.
 */
function useSubtitleChoice(
  askFromHere: ReturnType<typeof asker>,
  subtitleTrack: number | undefined,
) {
  const [shownSubtitle, setShownSubtitle] = useState<string>();
  const chooseSubtitle = (video: HTMLVideoElement, choice: SubtitleChoice) => {
    setShownSubtitle(typeof choice === "string" ? choice : undefined);
    const burnIn = typeof choice === "number" ? choice : undefined;
    if (burnIn !== subtitleTrack) askFromHere(video, { subtitleTrack: burnIn });
  };
  return { shownSubtitle, chooseSubtitle };
}

/**
 * The plan for Version `versionId` as `asking` says, asked for once and never reused after. A
 * Server too busy to Transcode it gives a plan that can't play, saying so.
 */
function usePlan(versionId: bigint, asking: Asking) {
  const transport = useTransport();
  const { failedMethods, startPositionMs, audioTrack, playbackLimit, subtitleTrack } = asking;
  return useSuspenseQuery({
    queryKey: [
      "planPlayback",
      String(versionId),
      failedMethods,
      String(startPositionMs),
      audioTrack,
      playbackLimit,
      subtitleTrack,
    ],
    queryFn: async () => {
      const deviceDecoding = await findDeviceDecoding();
      const input = { versionId, deviceDecoding, ...asking };
      try {
        const response = await callUnaryMethod(
          transport,
          PlaybackService.method.planPlayback,
          input,
        );
        return create(PlaybackPlanSchema, response.plan);
      } catch (error) {
        const { reason, params } = refusalOf(error);
        if (reason !== RefusalReason.SERVER_BUSY) throw error;
        return create(PlaybackPlanSchema, { reasons: [serverBusy(params["maxTranscodes"])] });
      }
    },
    staleTime: "static",
    gcTime: 0,
  }).data;
}

/** Why the Server, running `maxTranscodes` Transcodes at once, can't play a Version here now. */
function serverBusy(maxTranscodes = "") {
  const most = maxTranscodes === "" ? "" : ` (${maxTranscodes})`;
  return `The Server is busy: only a Transcode plays this here, and it’s already running as many as it’s set to run at once${most}. Try again once one ends.`;
}

/**
 * Where `video` stands in milliseconds, or `startPositionMs` if it never loaded far enough to have
 * started there.
 */
function resumeFromMs(video: HTMLVideoElement, startPositionMs: bigint) {
  if (video.readyState < HTMLMediaElement.HAVE_METADATA) return startPositionMs;
  return toMs(video.currentTime);
}
