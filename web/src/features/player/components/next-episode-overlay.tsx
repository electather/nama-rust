import type { LinkOptions } from "@tanstack/react-router";

import { PlayNowButton } from "@/features/player/components/play-now-button";
import { StillWatching } from "@/features/player/components/still-watching";
import type { useNextEpisode } from "@/features/player/hooks/use-next-episode";
import { clock } from "@/features/player/lib/video-actions";
import { episodeName } from "@/shared/lib/title-facts";
import { Button } from "@/shared/ui/button";

/** The countdown to the next Episode, or "Are you still watching?", over the player. */
export function NextEpisodeOverlay({
  nextEpisode,
  back,
}: {
  nextEpisode: ReturnType<typeof useNextEpisode>;
  back: LinkOptions;
}) {
  const { asking, keepWatching, countdown, secondsLeft, playNext, cancel } = nextEpisode;
  if (asking) {
    return <StillWatching back={back} onKeepWatching={keepWatching} />;
  }
  if (!countdown) {
    return null;
  }
  const { episode } = countdown;
  return (
    // Outside the controls, so it stays while they're hidden. It floats over the video, so it gets a
    // shadow (DESIGN.md, "Elevation").
    <aside
      aria-label="Next Episode"
      className="absolute inset-x-4 bottom-32 z-30 rounded-xl border bg-popover p-4 text-popover-foreground shadow-lg sm:inset-x-auto sm:inset-e-6 sm:w-80"
    >
      <p className="text-sm text-muted-foreground">
        {episodeName(episode.seasonNumber, episode.episodeNumber, "full")}
      </p>
      <p className="truncate font-medium">{episode.title}</p>
      <p className="mt-1 text-sm text-muted-foreground tabular-nums">
        Starts in {clock(Math.ceil(secondsLeft))}
      </p>
      <div className="mt-4 flex gap-2">
        <PlayNowButton onPlayNow={playNext} />
        <Button variant="ghost" onClick={cancel}>
          Cancel
        </Button>
      </div>
    </aside>
  );
}
