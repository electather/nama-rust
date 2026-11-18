import type { Dispatch, SetStateAction } from "react";

import { type Asking, type Choice, honoured, resumeFromMs } from "@/features/player/lib/asking";
import type { PlaybackLimit, PlaybackPlan } from "@/gen/nama/v1/playback_pb";

/**
 * What asks for a new plan with the User's choice, on top of what `plan` honoured, from where the
 * `<video>` stands, or else from `startPositionMs`, by `setAsking`.
 */
export function asker(
  setAsking: Dispatch<SetStateAction<Asking>>,
  plan: PlaybackPlan,
  startPositionMs: bigint,
) {
  return (video: HTMLVideoElement, choice: Choice) => {
    // As for a failure, so it isn't heard behind the loading screen.
    video.pause();
    const resumeFrom = resumeFromMs(video, startPositionMs);
    setAsking((before) => ({
      ...before,
      ...honoured(plan, before),
      ...choice,
      justChose: choice,
      startPositionMs: resumeFrom,
      replacing: plan.id,
      asks: before.asks + 1,
    }));
  };
}

/** `chooseAudio` and `chooseLimit`, which `askFromHere` with the User's choice. */
export function choosers(askFromHere: ReturnType<typeof asker>) {
  return {
    chooseAudio: (video: HTMLVideoElement, place: number) => {
      askFromHere(video, { audioTrack: place });
    },
    chooseLimit: (video: HTMLVideoElement, limit: PlaybackLimit) => {
      askFromHere(video, { playbackLimit: limit });
    },
  };
}
