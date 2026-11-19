import { Link, type LinkOptions } from "@tanstack/react-router";

import { EpisodeStill } from "@/features/player/components/episode-still";
import { EpisodeText } from "@/features/player/components/episode-text";
import type { Episode } from "@/gen/nama/v1/browse_pb";

/**
 * An Episode's Still, number and title, as a link that plays it. `played`, how far into
 * it the video is, is there only for the one playing, which is lit.
 */
export function EpisodeRow({
  episode,
  link,
  played,
  onPick,
}: {
  episode: Episode;
  link: LinkOptions;
  played: number | undefined;
  onPick: () => void;
}) {
  const playing = played !== undefined;
  return (
    <Link
      {...link}
      aria-current={playing ? "page" : undefined}
      onClick={onPick}
      className="group/row flex flex-none items-center gap-3 rounded-lg p-2 outline-none select-none hover:bg-muted focus-visible:ring-3 focus-visible:ring-ring/50 aria-[current=page]:bg-accent aria-[current=page]:inset-ring aria-[current=page]:inset-ring-primary/60"
    >
      <EpisodeStill episode={episode} played={played} />
      <EpisodeText episode={episode} playing={playing} />
    </Link>
  );
}
