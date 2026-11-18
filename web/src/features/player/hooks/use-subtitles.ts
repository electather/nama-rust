import { type RefObject, useEffect, useState } from "react";

import { clearOfControls } from "@/features/player/lib/clear-of-controls";
import type { SubtitleState } from "@/features/player/lib/subtitle-render";

/**
 * Shows the `<video>`'s `<track>` at the URL `shown`, or none, and says how each shown so far is
 * getting on. Showing one only changes which shows, never the plan or what the `<video>` plays.
 */
export function useSubtitles(
  videoRef: RefObject<HTMLVideoElement | null>,
  shown: string | undefined,
): Partial<Record<string, SubtitleState>> {
  const [states, setStates] = useState<Partial<Record<string, SubtitleState>>>({});
  useEffect(() => {
    // A `<track>`'s `default` counts only when the `<video>` first loads, so its mode is set.
    for (const element of videoRef.current?.querySelectorAll("track") ?? []) {
      element.track.mode = element.getAttribute("src") === shown ? "showing" : "disabled";
    }
  }, [videoRef, shown]);
  useEffect(() => {
    const video = videoRef.current;
    // A `<track>`'s load and error don't bubble, so the `<video>` catches them on the way down.
    const settle =
      (state: SubtitleState) =>
      ({ target }: Event) => {
        if (!(target instanceof HTMLTrackElement)) {
          return;
        }
        clearOfControls(target.track);
        setStates((before) => ({ ...before, [target.getAttribute("src") ?? ""]: state }));
      };
    const [onLoad, onError] = [settle("ready"), settle("failed")];
    video?.addEventListener("load", onLoad, true);
    video?.addEventListener("error", onError, true);
    return () => {
      video?.removeEventListener("load", onLoad, true);
      video?.removeEventListener("error", onError, true);
    };
  }, [videoRef]);
  // Shown and not yet settled is loading.
  return shown === undefined ? states : { [shown]: "loading", ...states };
}
