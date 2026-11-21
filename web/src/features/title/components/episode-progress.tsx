import { CircleCheck } from "lucide-react";

import { playedNotStarted } from "@/features/title/lib/episode-card-label";
import type { Episode } from "@/gen/nama/v1/browse_pb";
import { PlayProgress } from "@/shared/components/backdrop-card/play-progress";
import { remainingOf, runtimeOf } from "@/shared/lib/title-facts";

/**
 * An Episode card's last line: its runtime, with "Watched" once played, or once started ▶ and a
 * bar of how far in you are beside what's left.
 */
export function EpisodeProgress({ episode: { watchState, runtimeMinutes } }: { episode: Episode }) {
  const runtime = runtimeOf(runtimeMinutes, "short");
  if (playedNotStarted(watchState)) {
    return (
      <span aria-hidden className="mt-1 flex items-center gap-2.5 text-sm tabular-nums">
        {runtime}
        <span className="flex items-center gap-1">
          <CircleCheck className="size-3.5" />
          Watched
        </span>
      </span>
    );
  }
  return (
    <span className="mt-1">
      <PlayProgress state={watchState} detail={remainingOf(watchState) ?? runtime} quiet />
    </span>
  );
}
