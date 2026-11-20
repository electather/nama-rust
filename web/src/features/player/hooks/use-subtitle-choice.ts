import { useState } from "react";

import type { SubtitleChoice } from "@/features/player/lib/asking";
import type { asker } from "@/features/player/lib/plan-choices";
import type { PlaybackPlan } from "@/gen/nama/v1/playback_pb";

/** The subtitle the Device shows itself: by URL, or by title for one downloaded since the plan. */
type Shown = { url: string | undefined } | { title: string };

/**
 * The subtitle the Device shows itself, by URL, kept here so the `<video>` a new plan plays in
 * shows it too, and `chooseSubtitle`, which shows one, and will `askFromHere` when what's burned
 * in changes from `plan`'s. Only the first plan says which it starts with: after that it's the
 * User's. `chooseDownloaded` asks for a plan with the Track just downloaded, titled `title`, and
 * shows it.
 */
export function useSubtitleChoice(askFromHere: ReturnType<typeof asker>, plan: PlaybackPlan) {
  const [shown, setShown] = useState<Shown>(() => ({
    url: plan.subtitles.find(({ place }) => place === plan.startingSubtitle)?.url,
  }));
  const shownSubtitle =
    "url" in shown
      ? shown.url
      : plan.subtitles.find(({ track }) => track?.title === shown.title)?.url;
  const chooseSubtitle = (video: HTMLVideoElement, choice: SubtitleChoice) => {
    setShown({ url: typeof choice === "string" ? choice : undefined });
    const burnIn = typeof choice === "number" ? choice : undefined;
    if (burnIn !== plan.subtitleTrack) {
      askFromHere(video, { subtitleTrack: burnIn, noSubtitles: burnIn === undefined });
    }
  };
  const chooseDownloaded = (video: HTMLVideoElement, title: string) => {
    setShown({ title });
    // Only a new plan lists the new Track, which the Device shows, so none is burned in.
    askFromHere(video, { subtitleTrack: undefined, noSubtitles: true });
  };
  return { shownSubtitle, chooseSubtitle, chooseDownloaded };
}
