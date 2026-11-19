import type { Episode } from "@/gen/nama/v1/browse_pb";
import { Still } from "@/shared/components/still";
import { episodeName } from "@/shared/lib/title-facts";

/** The next Episode's Still, which Episode it is, its title, and how long until it starts. */
export function NextEpisodeFacts({
  episode,
  secondsLeft,
}: {
  episode: Episode;
  secondsLeft: number;
}) {
  return (
    <div className="flex items-center gap-3.5">
      <span className="w-35 flex-none">
        <Still url={episode.stillUrl} thumbhash={episode.stillThumbhash} sizes="140px" />
      </span>
      <p className="flex min-w-0 flex-1 flex-col gap-0.5 text-xs">
        <span>Next · {episodeName(episode.seasonNumber, episode.episodeNumber, "short")}</span>
        <span className="text-base font-medium">{episode.title}</span>
        <span role="timer" className="tabular-nums">
          Starts in {Math.ceil(secondsLeft)}s
        </span>
      </p>
    </div>
  );
}
