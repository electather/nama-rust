import { StillProgress } from "@/features/player/components/still-progress";
import type { Episode } from "@/gen/nama/v1/browse_pb";
import { Still } from "@/shared/components/still";
import { PlayedMark } from "@/shared/components/watch-state/played-mark";
import { watchedPercent } from "@/shared/lib/watch-progress";

/**
 * An Episode row's Still, marked once it's played and showing how far in you are, from `played`
 * for the one playing and else from its Watch State.
 */
export function EpisodeStill({
  episode,
  played,
}: {
  episode: Episode;
  played: number | undefined;
}) {
  return (
    <span className="w-28 flex-none">
      <Still url={episode.stillUrl} thumbhash={episode.stillThumbhash} sizes="112px">
        <PlayedMark state={played === undefined ? episode.watchState : undefined} />
        <StillProgress
          percent={played === undefined ? watchedPercent(episode.watchState) : played * 100}
        />
      </Still>
    </span>
  );
}
