import { type Dispatch, type SetStateAction, useCallback, useState } from "react";

import { type Asking, honoured, resumeFromMs } from "@/features/player/lib/asking";
import type { PlaybackPlan } from "@/gen/nama/v1/playback_pb";

/**
 * `onCantPlay`, which asks for a new plan without `plan`'s method, which just failed, by
 * `setAsking`, or else sets `gaveUp` if `asking` already ruled it out.
 */
export function useCantPlay(
  plan: PlaybackPlan,
  { failedMethods, startPositionMs, asks }: Asking,
  setAsking: Dispatch<SetStateAction<Asking>>,
) {
  const [gaveUp, setGaveUp] = useState(false);
  // Stable until the plan changes, so hls.js isn't restarted each time the player renders.
  const onCantPlay = useCallback(
    (video: HTMLVideoElement) => {
      // So a failure that leaves its buffer playing isn't heard behind the loading screen.
      video.pause();
      // Read here: React may run the updater later, when the `<video>` has moved on.
      const resumeFrom = resumeFromMs(video, startPositionMs);
      if (failedMethods.includes(plan.method)) {
        setGaveUp(true);
      }
      // hls.js and the `<video>` may both report one failure: only the first asks again. A plan
      // already being replaced fails once the Server stops it, which asks nothing.
      else {
        setAsking((before) =>
          before.asks !== asks || before.failedMethods.includes(plan.method)
            ? before
            : {
                ...before,
                ...honoured(plan, before),
                failedMethods: [...before.failedMethods, plan.method],
                startPositionMs: resumeFrom,
                replacing: plan.id,
                asks: before.asks + 1,
              },
        );
      }
    },
    [asks, failedMethods, plan, setAsking, startPositionMs],
  );
  return { gaveUp, onCantPlay };
}
