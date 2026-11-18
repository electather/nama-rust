import { useState } from "react";

import type { SubtitleChoice } from "@/features/player/lib/asking";
import type { asker } from "@/features/player/lib/plan-choices";
import type { PlaybackPlan } from "@/gen/nama/v1/playback_pb";

/**
 * The subtitle the Device shows itself, by URL, kept here so the `<video>` a new plan plays in
 * shows it too, and `chooseSubtitle`, which shows one, and will `askFromHere` when what's burned
 * in changes from `plan`'s. Only the first plan says which it starts with: after that it's the
 * User's.
 */
export function useSubtitleChoice(askFromHere: ReturnType<typeof asker>, plan: PlaybackPlan) {
  const [shownSubtitle, setShownSubtitle] = useState(
    () => plan.subtitles.find(({ place }) => place === plan.startingSubtitle)?.url,
  );
  const chooseSubtitle = (video: HTMLVideoElement, choice: SubtitleChoice) => {
    setShownSubtitle(typeof choice === "string" ? choice : undefined);
    const burnIn = typeof choice === "number" ? choice : undefined;
    if (burnIn !== plan.subtitleTrack) {
      askFromHere(video, { subtitleTrack: burnIn, noSubtitles: burnIn === undefined });
    }
  };
  return { shownSubtitle, chooseSubtitle };
}
