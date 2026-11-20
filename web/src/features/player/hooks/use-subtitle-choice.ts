import { useState } from "react";

import type { SubtitleChoice } from "@/features/player/lib/asking";
import type { asker } from "@/features/player/lib/plan-choices";
import type { Track } from "@/gen/nama/v1/browse_pb";
import type { FoundSubtitle, PlaybackPlan } from "@/gen/nama/v1/playback_pb";

/**
 * The subtitle the Device shows itself: by URL, or as found for one downloaded since the plan,
 * keeping the URL shown `before` for while no Track of the plan is the download.
 */
type Shown =
  | { url: string | undefined }
  | { downloaded: FoundSubtitle; before: string | undefined };

/**
 * The subtitle the Device shows itself, by URL, kept here so the `<video>` a new plan plays in
 * shows it too, and `chooseSubtitle`, which shows one, and will `askFromHere` when what's burned
 * in changes from `plan`'s. Only the first plan says which it starts with: after that it's the
 * User's. `chooseDownloaded` asks for a plan with the Track just downloaded from what a search
 * found, and shows it.
 */
export function useSubtitleChoice(askFromHere: ReturnType<typeof asker>, plan: PlaybackPlan) {
  const [shown, setShown] = useState<Shown>(() => ({
    url: plan.subtitles.find(({ place }) => place === plan.startingSubtitle)?.url,
  }));
  const shownSubtitle =
    "url" in shown
      ? shown.url
      : (plan.subtitles.find(({ track }) => isTrackOf(track, shown.downloaded))?.url ??
        shown.before);
  const chooseSubtitle = (video: HTMLVideoElement, choice: SubtitleChoice) => {
    setShown({ url: typeof choice === "string" ? choice : undefined });
    const burnIn = typeof choice === "number" ? choice : undefined;
    if (burnIn !== plan.subtitleTrack) {
      askFromHere(video, { subtitleTrack: burnIn, noSubtitles: burnIn === undefined });
    }
  };
  const chooseDownloaded = (video: HTMLVideoElement, downloaded: FoundSubtitle) => {
    setShown({ downloaded, before: shownSubtitle });
    // Only a new plan lists the new Track, which the Device shows, so none is burned in.
    askFromHere(video, { subtitleTrack: undefined, noSubtitles: true });
  };
  return { shownSubtitle, chooseSubtitle, chooseDownloaded };
}

/**
 * Whether `track` is the one the Server made of `found`, which it names, tags and marks SDH as
 * found. ponytail: a file downloaded before under another name keeps that name, and one the
 * Device can't draw is only burned in, so neither shows and the one shown before stays; match by
 * place once `DownloadSubtitle` answers with it.
 */
function isTrackOf(track: Track | undefined, { name, language, sdh }: FoundSubtitle) {
  return track?.title === name && track.language === language && track.sdh === sdh;
}
