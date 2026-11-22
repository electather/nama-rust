import { Link, type LinkOptions } from "@tanstack/react-router";
import { SkipForward } from "lucide-react";
import { buttonVariants } from "nama-ui/components/button";
import { useId } from "react";

import { NextEpisodePeek } from "@/features/player/components/next-episode-peek";
import type { Episode } from "@/gen/nama/v1/browse_pb";

/**
 * A link that plays the next Episode now, previewing it while hovered or focused. Not on phones,
 * where the bar has no room for it and the Episodes panel lists the next Episode.
 */
export function NextEpisodeButton({
  episode,
  link,
}: {
  episode: Episode;
  link: (episode: Episode) => LinkOptions;
}) {
  const peekId = useId();
  return (
    <span className="group/next relative hidden sm:flex">
      <Link
        {...link(episode)}
        aria-label={`Next Episode: ${episode.title}`}
        aria-describedby={peekId}
        className={buttonVariants({ variant: "overlay", size: "icon" })}
      >
        <SkipForward aria-hidden className="fill-current" />
      </Link>
      <NextEpisodePeek id={peekId} episode={episode} />
    </span>
  );
}
