import { type RefObject, useEffect, useState } from "react";

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
    const video = videoRef.current;
    // A `<track>`'s `default` counts only when the `<video>` first loads, so its mode is set.
    const keep = () => {
      for (const element of video?.querySelectorAll("track") ?? []) {
        // The mode it already has fires no `change`, so this ends.
        element.track.mode = element.getAttribute("src") === shown ? "showing" : "disabled";
      }
    };
    keep();
    // And again when the browser shows one of its own accord, as Chrome does the one the system's
    // captions setting prefers, such as an SDH, after this has set them.
    video?.textTracks.addEventListener("change", keep);
    return () => {
      video?.textTracks.removeEventListener("change", keep);
    };
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
