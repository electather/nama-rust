import { useState } from "react";

import { usePlan } from "@/features/player/data/use-plan";
import { useCantPlay } from "@/features/player/hooks/use-cant-play";
import { useSubtitleChoice } from "@/features/player/hooks/use-subtitle-choice";
import { type Asking, firstAsking } from "@/features/player/lib/asking";
import { asker, choosers } from "@/features/player/lib/plan-choices";
import type { WatchState } from "@/gen/nama/v1/watch_state_pb";

/**
 * The Server's Playback Plan for Version `versionId` on this Device, the position in milliseconds
 * it starts from, at first where `watchState` stopped, and `onCantPlay` for when the browser fails
 * to play it. `onCantPlay` asks for a new plan from where the `<video>` stands, without the method
 * that failed, or else sets `gaveUp`: only once per method, so a Server that plans it again can't
 * make the player ask forever. `chooseAudio` asks for a new plan from there with the audio Track
 * at a place in the Version's Tracks, and `chooseLimit` with a Playback Limit. `chooseSubtitle`
 * shows a subtitle the Device shows itself, which `shownSubtitle` then holds, starting as the first
 * plan's `startingSubtitle`, or asks for a new plan from there with a subtitle Track burned in. The
 * plan says which of these it honoured, which may drop `justChose` (ADR 0031).
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
  const [asking, setAsking] = useState<Asking>(() => firstAsking(watchState));
  const { startPositionMs } = asking;
  const plan = usePlan(versionId, asking);
  const askFromHere = asker(setAsking, plan, startPositionMs);
  const subtitles = useSubtitleChoice(askFromHere, plan);
  const failing = useCantPlay(plan, asking, setAsking);
  const { justChose, asks } = asking;
  return {
    versionId,
    plan,
    startPositionMs,
    justChose,
    asks,
    ...failing,
    ...choosers(askFromHere),
    ...subtitles,
  };
}
