import type { LinkOptions } from "@tanstack/react-router";

import { NextEpisodeCard } from "@/features/player/components/next-episode-card";
import { StillWatching } from "@/features/player/components/still-watching";
import type { useNextEpisode } from "@/features/player/hooks/use-next-episode";

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
  return (
    <NextEpisodeCard
      episode={countdown.episode}
      secondsLeft={secondsLeft}
      onPlayNext={playNext}
      onCancel={cancel}
    />
  );
}
